use anyhow::anyhow;
use libcontainer::oci_spec::runtime::Spec;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError, EMPTY};
use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;

use libloading::{Library, Symbol};
use nix::fcntl::{openat, OFlag};
use nix::sys::stat::Mode;
use nix::unistd::write;
use std::os::raw::{c_char, c_int};

const EXECUTOR_NAME: &str = "libkrun";

#[derive(Clone)]
pub struct LibkrunExecutor {}

impl Executor for LibkrunExecutor {
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if !can_handle(spec) {
            return Err(ExecutorError::CantHandle(EXECUTOR_NAME));
        }

        // Spec を出力
        // tracing::debug!("Spec: {:#?}", spec);

        // 仮のパス設定（必要に応じて実際のパスに変更）
        // let state_dir = PathBuf::from("");
        // let container_spec_path = state_dir.join("config.json");
        // let rootfs = Some(PathBuf::from("/var/lib/containers/rootfs"));

        let rootfs_path = spec
            .root()
            .as_ref()
            .ok_or_else(|| anyhow!("spec.root is missing"))
            .map_err(|e| ExecutorError::Other(format!("failed to get rootfs: {}", e)))?
            .path();
        let rootfs = PathBuf::from(rootfs_path);
        println!("rootfs path: {}", rootfs.display());

        let json_spec = serde_json::to_string_pretty(spec).map_err(|e| {
            ExecutorError::Other(format!("failed to serialize spec to JSON: {}", e))
        })?;
        println!("spec as JSON:\n{}", json_spec);

        // write_config_to_rootfs(&rootfs, &json_spec)?;

        // // 初期化フェーズ: BeforeMounts ここでやっても意味ない
        // configure_container(
        //     true,
        //     &spec,
        //     // &state_dir,
        //     // &container_spec_path,
        //     // rootfs.as_deref(),
        // )
        // .map_err(|e| ExecutorError::Other(format!("configure_container failed: {}", e)))?;

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
        let lib = unsafe {
            Library::new("/usr/local/lib64/libkrun.so")
                .map_err(|e| ExecutorError::Other(format!("failed to load libkrun.so: {}", e)))?
        };

        unsafe {
            let krun_create_ctx: Symbol<unsafe extern "C" fn() -> c_int> =
                lib.get(b"krun_create_ctx").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_create_ctx: {}", e))
                })?;
            let krun_set_vm_config: Symbol<unsafe extern "C" fn(c_int, c_int, c_int) -> c_int> =
                lib.get(b"krun_set_vm_config").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_set_vm_config: {}", e))
                })?;
            let krun_set_root: Symbol<unsafe extern "C" fn(c_int, *const c_char) -> c_int> =
                lib.get(b"krun_set_root").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_set_root: {}", e))
                })?;
            let krun_set_exec: Symbol<
                unsafe extern "C" fn(c_int, *const c_char, c_int, *const *const c_char) -> c_int,
            > = lib.get(b"krun_set_exec").map_err(|e| {
                ExecutorError::Other(format!("failed to load krun_set_exec: {}", e))
            })?;
            let krun_start_enter: Symbol<unsafe extern "C" fn(c_int) -> c_int> =
                lib.get(b"krun_start_enter").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_start_enter: {}", e))
                })?;

            let ctx_id = krun_create_ctx();
            krun_set_vm_config(ctx_id, 1, 512);
            let root = CString::new("rootfs").map_err(|e| {
                ExecutorError::Other(format!("failed to create CString for rootfs: {}", e))
            })?;
            krun_set_root(ctx_id, root.as_ptr());

            let bin = CString::new("/bin/sh").map_err(|e| {
                ExecutorError::Other(format!("failed to create CString for /bin/sh: {}", e))
            })?;
            let envp: [*const c_char; 1] = [std::ptr::null()];
            krun_set_exec(ctx_id, bin.as_ptr(), 0, envp.as_ptr());

            let ret = krun_start_enter(ctx_id);
            if ret < 0 {
                eprintln!("krun_start_enter failed with return code: {}", ret);
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

pub fn get_executor() -> LibkrunExecutor {
    LibkrunExecutor {}
}

fn can_handle(spec: &Spec) -> bool {
    true
}

//ここから下は適当

// use anyhow::{anyhow, Context, Result};
// use std::fs;
// use std::os::unix::fs::PermissionsExt;
// use std::os::unix::prelude::OpenOptionsExt;
// use std::path::{Path, PathBuf};

// #[derive(Debug)]
// struct Device {
//     path: &'static str,
//     kind: char,
//     major: u32,
//     minor: u32,
//     mode: u32,
// }

// const KVM_DEVICE: Device = Device {
//     path: "/dev/kvm",
//     kind: 'c',
//     major: 10,
//     minor: 232,
//     mode: 0o666,
// };

// const SEV_DEVICE: Device = Device {
//     path: "/dev/sev",
//     kind: 'c',
//     major: 10,
//     minor: 124,
//     mode: 0o666,
// };

// #[derive(PartialEq)]
// enum HandlerPhase {
//     BeforeMounts,
//     AfterMounts,
// }

// libkrun_configure_container
fn configure_container(
    handle_sev_present: bool,
    spec: &Spec, // // phase: HandlerPhase,
                 // state_dir: &Path,
                 // container_spec_path: &Path,
                 // rootfs: Option<&Path>,
) -> Result<(), anyhow::Error> {
    // match phase {
    // HandlerPhase::BeforeMounts => {
    tracing::debug!("Spec: {:#?}", spec);

    // let config_path = state_dir.join("config.json");
    // let config_data = fs::read(&config_path)
    //     .map_err(|e| anyhow!("failed to read config from {}: {}", config_path.display(), e))?;

    // let krun_config_path = rootfs
    //     .map(|r| r.join("krun_config.json"))
    //     .ok_or_else(|| anyhow!("rootfs not provided"))?;
    // fs::write(&krun_config_path, config_data)
    //     .map_err(|e| anyhow::anyhow!("failed to write krun_config.json to {}: {}", krun_config_path.display(), e))?;

    // }
    // // HandlerPhase::AfterMounts => {
    //     let rootfs_path = rootfs.ok_or_else(|| anyhow!("missing rootfs path"))?;
    //     let dev_path = rootfs_path.join("dev");

    //     create_dev_node(&dev_path, &KVM_DEVICE, in_user_ns)?;
    //     if handle_sev_present {
    //         create_dev_node(&dev_path, &SEV_DEVICE, in_user_ns)?;
    //     }
    // }
    // }
    Ok(())
}

// fn write_config_to_rootfs(&self, rootfs: &PathBuf, json_spec: &str) -> Result<(), ExecutorError> {
//     let krun_config_file = ".krun_config.json";
//     let krun_config_path = rootfs.join(krun_config_file);

//     let test_path = rootfs.join(".krun_config.json");
//     println!("writing .krun_config.json to: {}", test_path.display());
//     fs::write(&test_path, json_spec)
//         .map_err(|e| ExecutorError::Other(format!("fs::write failed: {}", e)))?;

//     // // 親ディレクトリのfdを取得
//     // let rootfs_fd = openat(
//     //     None,
//     //     rootfs,
//     //     OFlag::O_DIRECTORY | OFlag::O_RDONLY,
//     //     Mode::empty(),
//     // )
//     // .map_err(|e| ExecutorError::Other(format!("failed to open rootfs dir: {}", e)))?;

//     // // ファイルを openat で安全に作成 (O_NOFOLLOW)
//     // let fd = openat(
//     //     Some(rootfs_fd),
//     //     CString::new(krun_config_file).unwrap().as_c_str(),
//     //     OFlag::O_CREAT | OFlag::O_WRONLY | OFlag::O_TRUNC | OFlag::O_NOFOLLOW,
//     //     Mode::from_bits_truncate(0o644),
//     // )
//     // .map_err(|e| ExecutorError::Other(format!("failed to open krun_config.json: {}", e)))?;

//     // // ファイルに書き込み
//     // unsafe {
//     //     write(BorrowedFd::borrow_raw(fd), json_spec.as_bytes()).map_err(|e| {
//     //         ExecutorError::Other(format!("failed to write krun_config.json: {}", e))
//     //     })?;
//     // }
//     Ok(())
// }
