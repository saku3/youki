use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, io};

use libcontainer::error::MissingSpecError;
use libcontainer::oci_spec::runtime::{
    LinuxBuilder, LinuxDevice, LinuxDeviceBuilder, LinuxDeviceCgroup, LinuxDeviceCgroupBuilder,
    LinuxDeviceType, Spec,
};
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError, EMPTY};
use libloading::{Library, Symbol};
use nix::errno::Errno;
use nix::sys::stat::{major, minor, stat};
use once_cell::sync::OnceCell;

const EXECUTOR_NAME: &str = "libkrun";
const LIBKRUN_NAME: &str = "libkrun.so.1";

#[derive(Clone)]
pub struct LibkrunExecutor {
    lib: Arc<OnceCell<Arc<Library>>>,
    ctx_id: Arc<OnceCell<c_int>>,
}

impl LibkrunExecutor {
    fn get_or_load_lib(&self) -> Result<Arc<Library>, ExecutorError> {
        if let Some(lib) = self.lib.get() {
            return Ok(lib.clone());
        }
        let lib = unsafe { Library::new(LIBKRUN_NAME) }
            .map_err(|e| ExecutorError::Other(format!("failed to load {LIBKRUN_NAME}: {e}")))?;
        let lib = Arc::new(lib);
        let _ = self.lib.set(lib.clone());
        Ok(lib)
    }

    fn lib_loaded(&self) -> Result<&Arc<Library>, ExecutorError> {
        self.lib
            .get()
            .ok_or_else(|| ExecutorError::Other("libkrun not preloaded".into()))
    }

    fn set_ctx_id(&self, value: c_int) -> Result<(), ExecutorError> {
        self.ctx_id
            .set(value)
            .map_err(|_| ExecutorError::Other("ctx_id already initialized".into()))
    }

    fn get_ctx_id(&self) -> c_int {
        *self.ctx_id.get().expect("ctx_id not initialized")
    }
}

pub fn get_executor() -> LibkrunExecutor {
    LibkrunExecutor {
        lib: Arc::new(OnceCell::new()),
        ctx_id: Arc::new(OnceCell::new()),
    }
}

impl Executor for LibkrunExecutor {
    fn pre_exec(&self, spec: Spec) -> Result<Spec, ExecutorError> {
        if !can_handle(&spec) {
            return Err(ExecutorError::CantHandle(EXECUTOR_NAME));
        }
        tracing::debug!("executing libkrun pre executer");

        let spec = configure_spec_for_libkrun(spec)
            .map_err(|e| ExecutorError::Other(format!("configure_for_libkrun: {e}")))?;

        let lib = self.get_or_load_lib()?;
        let krun_create_ctx: Symbol<unsafe extern "C" fn() -> c_int> = unsafe {
            lib.get(b"krun_create_ctx").map_err(|e| {
                ExecutorError::Other(format!("failed to load krun_create_ctx: {}", e))
            })?
        };

        let ctx_id = unsafe { krun_create_ctx() };
        self.set_ctx_id(ctx_id)?;
        Ok(spec)
    }

    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if !can_handle(spec) {
            return Err(ExecutorError::CantHandle(EXECUTOR_NAME));
        }
        tracing::debug!("executing libkrun executer");

        let process = spec.process().as_ref();
        let args = process.and_then(|p| p.args().as_ref()).unwrap_or(&EMPTY);
        if args.is_empty() {
            tracing::error!("at least one process arg must be specified");
            return Err(ExecutorError::InvalidArg);
        }

        unsafe {
            let lib = self.lib_loaded()?;
            let ctx_id = self.get_ctx_id();

            let krun_set_vm_config: Symbol<unsafe extern "C" fn(c_int, c_int, c_int) -> c_int> =
                lib.get(b"krun_set_vm_config").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_set_vm_config: {}", e))
                })?;

            let krun_set_root: Symbol<unsafe extern "C" fn(c_int, *const c_char) -> c_int> =
                lib.get(b"krun_set_root").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_set_root: {}", e))
                })?;

            let krun_set_log_level: Symbol<unsafe extern "C" fn(c_int) -> c_int> =
                lib.get(b"krun_set_log_level").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_set_log_level: {}", e))
                })?;

            let krun_start_enter: Symbol<unsafe extern "C" fn(c_int) -> c_int> =
                lib.get(b"krun_start_enter").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_start_enter: {}", e))
                })?;

            let rc = krun_set_log_level(1);
            if rc < 0 {
                return Err(ExecutorError::Other(format!("krun_set_log_level rc={rc}")));
            }

            // TODO: Do not hardcode vCPU/RAM.
            let rc = krun_set_vm_config(ctx_id, 1, 512);
            if rc < 0 {
                return Err(ExecutorError::Other(format!("krun_set_vm_config rc={rc}")));
            }

            let root = CString::new("/")
                .map_err(|e| ExecutorError::Other(format!("CString::new for root failed: {e}")))?;
            let rc = krun_set_root(ctx_id, root.as_ptr());
            if rc < 0 {
                return Err(ExecutorError::Other(format!("krun_set_root rc={rc}")));
            }

            let ret = krun_start_enter(ctx_id);
            if ret < 0 {
                return Err(ExecutorError::Other(format!("krun_start_enter rc={ret}")));
            }
        }
        std::process::exit(0)
    }

    fn validate(&self, spec: &Spec) -> Result<(), ExecutorValidationError> {
        if !can_handle(spec) {
            return Err(ExecutorValidationError::CantHandle(EXECUTOR_NAME));
        }
        Ok(())
    }
}

fn can_handle(spec: &Spec) -> bool {
    if let Some(annotations) = spec.annotations() {
        if let Some(handler) = annotations.get("run.oci.handler") {
            return handler == "krun";
        }
    }

    false
}

#[derive(Debug, thiserror::Error)]
pub enum KrunError {
    #[error("{0}")]
    Other(String),
}

// add /dev/kvm to linux.device
pub fn modify_spec_device(spec: &mut Spec) -> Result<(), KrunError> {
    let mut linux = match spec.linux().clone() {
        Some(l) => l,
        None => LinuxBuilder::default()
            .build()
            .map_err(|e| KrunError::Other(format!("build default linux section: {e}")))?,
    };

    let (kvm_major, kvm_minor) = match stat_dev_numbers("/dev/kvm") {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            spec.set_linux(Some(linux));
            return Ok(());
        }
        Err(e) => return Err(KrunError::Other(format!("stat `/dev/kvm`: {e}"))),
    };

    let mut devices: Vec<LinuxDevice> = linux.devices().clone().unwrap_or_default();

    let exists = devices.iter().any(|d| d.path() == Path::new("/dev/kvm"));
    if !exists {
        devices.push(make_oci_spec_device(
            PathBuf::from("/dev/kvm"),
            LinuxDeviceType::C,
            kvm_major,
            kvm_minor,
            0o666u32,
            0u32,
            0u32,
        ));
        linux.set_devices(Some(devices));
    }

    spec.set_linux(Some(linux));
    Ok(())
}

// Add an allow rule for /dev/kvm to linux.resources.devices
// if resources.devices is None or empty, it's effectively permissive, so skip.
pub fn modify_spec_resource_device(spec: &mut Spec) -> Result<(), KrunError> {
    let mut linux = match spec.linux() {
        Some(l) => l.clone(),
        None => return Ok(()),
    };
    let mut res = match linux.resources() {
        Some(r) => r.clone(),
        None => return Ok(()),
    };
    let mut device_cgroups: Vec<LinuxDeviceCgroup> = match res.devices() {
        Some(v) if !v.is_empty() => v.clone(),
        _ => return Ok(()),
    };

    let (kvm_major, kvm_minor) = match stat_dev_numbers("/dev/kvm") {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(KrunError::Other(format!("stat `/dev/kvm`: {e}"))),
    };

    device_cgroups.push(make_oci_spec_dev_cgroup(
        LinuxDeviceType::C,
        kvm_major,
        kvm_minor,
        true,
        "rwm",
    ));
    res.set_devices(Some(device_cgroups));
    linux.set_resources(Some(res));
    spec.set_linux(Some(linux));

    Ok(())
}

fn stat_dev_numbers(path: &str) -> std::io::Result<(i64, i64)> {
    match stat(Path::new(path)) {
        Ok(st) => Ok((major(st.st_rdev) as i64, minor(st.st_rdev) as i64)),
        Err(Errno::ENOENT) => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
    }
}

pub fn write_krun_config(rootfs: &Path, json_spec: &str) -> Result<(), KrunError> {
    let krun_config_file = ".krun_config.json";
    let config_path = rootfs.join(krun_config_file);
    println!("writing .krun_config.json to: {}", config_path.display());

    // TODO atomic write
    // https://github.com/containers/crun/blob/main/src/libcrun/handlers/krun.c#L397
    fs::write(&config_path, json_spec)
        .map_err(|e| KrunError::Other(format!("fs::write failed: {}", e)))?;

    Ok(())
}

pub fn configure_spec_for_libkrun(mut spec: Spec) -> Result<Spec, KrunError> {
    let use_krun = {
        spec.annotations()
            .as_ref()
            .and_then(|a| a.get("run.oci.handler"))
            .map(|v| v == "krun")
            .unwrap_or(false)
    };

    if !use_krun {
        return Ok(spec);
    }

    let rootfs = spec
        .root()
        .as_ref()
        .ok_or(MissingSpecError::Root)
        .map_err(|e| KrunError::Other(format!("Missing root in spec: {e:?}")))?
        .path()
        .to_path_buf();
    modify_spec_device(&mut spec)
        .map_err(|e| KrunError::Other(format!("modify_spec_device: {e:?}")))?;
    modify_spec_resource_device(&mut spec)
        .map_err(|e| KrunError::Other(format!("modify_spec_resource_device: {e:?}")))?;

    let json_spec = serde_json::to_string_pretty(&spec)
        .map_err(|e| KrunError::Other(format!("failed to serialize spec to JSON: {}", e)))?;
    write_krun_config(&rootfs, &json_spec)
        .map_err(|e| KrunError::Other(format!("write_krun_config: {e:?}")))?;
    Ok(spec)
}

fn make_oci_spec_dev_cgroup(
    dev_type: LinuxDeviceType,
    major_num: i64,
    minor_num: i64,
    allow: bool,
    access: &str,
) -> LinuxDeviceCgroup {
    LinuxDeviceCgroupBuilder::default()
        .allow(allow)
        .typ(dev_type)
        .major(major_num)
        .minor(minor_num)
        .access(access.to_string())
        .build()
        .expect("device cgroup build")
}

fn make_oci_spec_device(
    path: impl Into<PathBuf>,
    dev_type: LinuxDeviceType,
    major_num: i64,
    minor_num: i64,
    file_mode: u32,
    uid: u32,
    gid: u32,
) -> LinuxDevice {
    LinuxDeviceBuilder::default()
        .typ(dev_type)
        .path(path.into())
        .major(major_num)
        .minor(minor_num)
        .file_mode(file_mode)
        .uid(uid)
        .gid(gid)
        .build()
        .expect("device node build")
}
