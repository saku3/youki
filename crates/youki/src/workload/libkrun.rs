use libcontainer::oci_spec::runtime::Spec;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError, EMPTY};

const EXECUTOR_NAME: &str = "libkrun";

#[derive(Clone)]
pub struct LibkrunExecutor {}

impl Executor for LibkrunExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if !can_handle(spec) {
            return Err(ExecutorError::CantHandle(EXECUTOR_NAME));
        }

        tracing::debug!("executing workload with libkrun handler");
        let process = spec.process().as_ref();

        let args = spec
            .process()
            .as_ref()
            .and_then(|p| p.args().as_ref())
            .unwrap_or(&EMPTY);
        if args.is_empty() {
            tracing::error!("at least one process arg must be specified");
            return Err(ExecutorError::InvalidArg);
        }

        // if !args[0].ends_with(".wasm") && !args[0].ends_with(".wat") {
        //     tracing::error!(
        //         "first argument must be a wasm or wat module, but was {}",
        //         args[0]
        //     );
        //     return Err(ExecutorError::InvalidArg);
        // }

        let mut cmd = args[0].clone();
        let stripped = args[0].strip_prefix(std::path::MAIN_SEPARATOR);
        if let Some(cmd_stripped) = stripped {
            cmd = cmd_stripped.to_string();
        }

        let envs: Vec<(String, String)> = process
            .and_then(|p| p.env().as_ref())
            .unwrap_or(&EMPTY)
            .iter()
            .filter_map(|e| {
                e.split_once('=')
                    .map(|kv| (kv.0.trim().to_string(), kv.1.trim().to_string()))
            })
            .collect();

        // let engine = Engine::default();
        // let module = Module::from_file(&engine, &cmd).map_err(|err| {
        //     tracing::error!(err = ?err, file = ?cmd, "could not load libkrun module from file");
        //     ExecutorError::Other("could not load libkrun module from file".to_string())
        // })?;

        std::process::exit(0)
    }

    fn validate(&self, spec: &Spec) -> Result<(), ExecutorValidationError> {
        if !can_handle(spec) {
            return Err(ExecutorValidationError::CantHandle(EXECUTOR_NAME));
        }

        Ok(())
    }
}

pub fn get_executor() -> LibkrunExecutor {
    LibkrunExecutor {}
}

fn can_handle(spec: &Spec) -> bool {
    true
    // if let Some(annotations) = spec.annotations() {
    //     if let Some(handler) = annotations.get("run.oci.handler") {
    //         return handler == "libkrun";
    //     }
    // }

    // false
}


//ここから下は適当

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::prelude::OpenOptionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Device {
    path: &'static str,
    kind: char,
    major: u32,
    minor: u32,
    mode: u32,
}

const KVM_DEVICE: Device = Device {
    path: "/dev/kvm",
    kind: 'c',
    major: 10,
    minor: 232,
    mode: 0o666,
};

const SEV_DEVICE: Device = Device {
    path: "/dev/sev",
    kind: 'c',
    major: 10,
    minor: 124,
    mode: 0o666,
};

#[derive(PartialEq)]
enum HandlerPhase {
    BeforeMounts,
    AfterMounts,
}

// libkrun_configure_container
fn configure_container(
    handle_sev_present: bool,
    phase: HandlerPhase,
    state_dir: &Path,
    container_spec_path: &Path,
    rootfs: Option<&Path>,
) -> Result<()> {
    match phase {
        HandlerPhase::BeforeMounts => {
            let config_path = state_dir.join("config.json");
            let config_data = fs::read(&config_path)
                .with_context(|| format!("failed to read config from {}", config_path.display()))?;
            let krun_config_path = rootfs
                .map(|r| r.join("krun_config.json"))
                .ok_or_else(|| anyhow!("rootfs not provided"))?;
            fs::write(&krun_config_path, config_data)
                .with_context(|| format!("failed to write krun_config.json to {}", krun_config_path.display()))?;
        }
        HandlerPhase::AfterMounts => {
            let rootfs_path = rootfs.ok_or_else(|| anyhow!("missing rootfs path"))?;
            let dev_path = rootfs_path.join("dev");
            let in_user_ns = check_user_namespace()?;

            create_dev_node(&dev_path, &KVM_DEVICE, in_user_ns)?;
            if handle_sev_present {
                create_dev_node(&dev_path, &SEV_DEVICE, in_user_ns)?;
            }
        }
    }
    Ok(())
}

fn check_user_namespace() -> Result<bool> {
    let data = fs::read_to_string("/proc/self/uid_map")?;
    Ok(data.lines().next().map_or(false, |l| l.trim() != "0 0 4294967295"))
}

fn create_dev_node(dev_dir: &Path, device: &Device, in_user_ns: bool) -> Result<()> {
    let dev_file = dev_dir.join(Path::new(device.path).file_name().unwrap());

    if dev_file.exists() {
        return Ok(());
    }

    if in_user_ns {
        // fallback: create empty file or bind mount
        fs::File::create(&dev_file)?;
    } else {
        use nix::sys::stat::{mknod, Mode, SFlag, makedev};
        use std::os::unix::ffi::OsStrExt;

        let kind = match device.kind {
            'c' => SFlag::S_IFCHR,
            'b' => SFlag::S_IFBLK,
            _ => return Err(anyhow!("unsupported device kind")),
        };

        let dev = makedev(device.major, device.minor);
        mknod(&dev_file, kind, Mode::from_bits_truncate(device.mode), dev)?;
    }

    Ok(())
}
