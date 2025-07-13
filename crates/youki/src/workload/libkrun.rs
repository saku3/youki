use anyhow::anyhow;
use libcontainer::oci_spec::runtime::Spec;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError, EMPTY};
use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;

use libloading::{Library, Symbol};
use nix::fcntl::{openat, OFlag};
use nix::sys::stat::Mode;
use std::os::raw::{c_char, c_int};

const EXECUTOR_NAME: &str = "libkrun";

#[derive(Clone)]
pub struct LibkrunExecutor {}

impl Executor for LibkrunExecutor {
    fn pre_exec(&self) -> Result<(), ExecutorError> {
        println!("pre_exec!!!");
        let lib = unsafe {
            Library::new("libkrun.so.1").map_err(|_| ExecutorError::Other("libloading error".to_string()))?
        };
        LIBKRUN
            .set(Arc::new(lib))
            .map_err(|_| ExecutorError::Other("libloading set error".to_string()))?;
        Ok(())
    }
    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if !can_handle(spec) {
            return Err(ExecutorError::CantHandle(EXECUTOR_NAME));
        }

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

        unsafe {
            let lib = get_libkrun();
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
use once_cell::sync::OnceCell;

static LIBKRUN: OnceCell<Arc<Library>> = OnceCell::new();

// pub fn preload_libkrun() -> Result<(), String> {
//     let lib = unsafe {
//         Library::new("libkrun.so.1").map_err(|e| format!("failed to preload libkrun: {e}"))?
//     };
//     LIBKRUN
//         .set(Arc::new(lib))
//         .map_err(|_| "already initialized".to_string())?;
//     Ok(())
// }

pub fn get_libkrun() -> Arc<Library> {
    LIBKRUN.get().expect("libkrun not preloaded").clone()
}

fn can_handle(spec: &Spec) -> bool {
    true
}

// libkrun_configure_container
fn configure_container(handle_sev_present: bool, spec: &Spec) -> Result<(), anyhow::Error> {
    tracing::debug!("Spec: {:#?}", spec);
    Ok(())
}
