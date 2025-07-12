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
pub struct LibkrunExecutor {
    pub krun_create_ctx: unsafe extern "C" fn() -> c_int,
    pub krun_set_vm_config: unsafe extern "C" fn(c_int, u8, u32) -> c_int,
    pub krun_set_root: Option<unsafe extern "C" fn(c_int, *const c_char) -> c_int>,
    pub krun_set_root_disk: Option<unsafe extern "C" fn(c_int, *const c_char) -> c_int>,
    pub krun_set_tee_config_file: Option<unsafe extern "C" fn(c_int, *const c_char) -> c_int>,
    pub krun_set_log_level: Option<unsafe extern "C" fn(u32) -> c_int>,
    pub krun_set_exec:
        Option<unsafe extern "C" fn(c_int, *const c_char, c_int, *const *const c_char) -> c_int>,
    pub krun_start_enter: unsafe extern "C" fn(c_int) -> c_int,

    _lib: Arc<Library>, // ✅ Arcで共有可能に
}

impl Executor for LibkrunExecutor {
    fn pre_exec(&self) -> Result<(), ExecutorError> {
        println!("pre_exec!!!");
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
            let ctx_id = (self.krun_create_ctx)();
            (self.krun_set_vm_config)(ctx_id, 1, 512);

            if let Some(set_root) = self.krun_set_root {
                let root = CString::new("rootfs").map_err(|e| {
                    ExecutorError::Other(format!("failed to create CString for rootfs: {}", e))
                })?;
                set_root(ctx_id, root.as_ptr());
            }

            let bin = CString::new("/bin/sh").map_err(|e| {
                ExecutorError::Other(format!("failed to create CString for /bin/sh: {}", e))
            })?;
            let envp: [*const c_char; 1] = [std::ptr::null()];
            if let Some(set_exec) = self.krun_set_exec {
                set_exec(ctx_id, bin.as_ptr(), 0, envp.as_ptr());
            } else {
                return Err(ExecutorError::Other(
                    "krun_set_exec is not available".to_string(),
                ));
            }
            let ret = (self.krun_start_enter)(ctx_id);
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
    let lib =
        Arc::new(unsafe { Library::new("libkrun.so.1").expect("Failed to load libkrun.so.1") });

    macro_rules! load_fn {
        ($name:literal, $ty:ty) => {
            unsafe {
                let symbol: Symbol<$ty> = lib
                    .get(concat!($name, "\0").as_bytes())
                    .expect(concat!("symbol not found: ", $name));
                *symbol
            }
        };
    }

    macro_rules! load_optional_fn {
        ($name:literal, $ty:ty) => {
            unsafe {
                match lib.get::<$ty>(concat!($name, "\0").as_bytes()) {
                    Ok(sym) => Some(*sym),
                    Err(_) => None,
                }
            }
        };
    }

    LibkrunExecutor {
        krun_create_ctx: load_fn!("krun_create_ctx", unsafe extern "C" fn() -> c_int),
        krun_set_vm_config: load_fn!(
            "krun_set_vm_config",
            unsafe extern "C" fn(c_int, u8, u32) -> c_int
        ),
        krun_set_root: load_optional_fn!(
            "krun_set_root",
            unsafe extern "C" fn(c_int, *const c_char) -> c_int
        ),
        krun_set_root_disk: load_optional_fn!(
            "krun_set_root_disk",
            unsafe extern "C" fn(c_int, *const c_char) -> c_int
        ),
        krun_set_tee_config_file: load_optional_fn!(
            "krun_set_tee_config_file",
            unsafe extern "C" fn(c_int, *const c_char) -> c_int
        ),
        krun_set_log_level: load_optional_fn!(
            "krun_set_log_level",
            unsafe extern "C" fn(u32) -> c_int
        ),
        krun_set_exec: load_optional_fn!(
            "krun_set_exec",
            unsafe extern "C" fn(c_int, *const c_char, c_int, *const *const c_char) -> c_int
        ),
        krun_start_enter: load_fn!("krun_start_enter", unsafe extern "C" fn(c_int) -> c_int),
        _lib: lib,
    }
}

fn can_handle(spec: &Spec) -> bool {
    true
}

// libkrun_configure_container
fn configure_container(handle_sev_present: bool, spec: &Spec) -> Result<(), anyhow::Error> {
    tracing::debug!("Spec: {:#?}", spec);
    Ok(())
}
