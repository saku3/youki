use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::{Arc, Mutex};

use libcontainer::oci_spec::runtime::Spec;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError, EMPTY};
use libloading::{Library, Symbol};
use once_cell::sync::Lazy;

const EXECUTOR_NAME: &str = "libkrun";
const LIBKRUN_NAME: &str = "libkrun.so.1";

// Lazy loading of libkrun
static LIBKRUN: Lazy<Option<Arc<Library>>> =
    Lazy::new(|| unsafe { Library::new(LIBKRUN_NAME).ok().map(Arc::new) });

// Lazy, mutable ctx_id
static CTX_ID: Lazy<Mutex<Option<c_int>>> = Lazy::new(|| Mutex::new(None));

fn get_libkrun() -> Arc<Library> {
    LIBKRUN.as_ref().expect("libkrun not preloaded").clone()
}

fn set_ctx_id(value: c_int) -> Result<(), ExecutorError> {
    let mut guard = CTX_ID.lock().unwrap();
    if guard.is_some() {
        return Err(ExecutorError::Other(
            "ctx_id already initialized".to_string(),
        ));
    }
    *guard = Some(value);
    Ok(())
}

fn get_ctx_id() -> c_int {
    let guard = CTX_ID.lock().unwrap();
    guard.expect("ctx_id not initialized. Call pre_exec() first.")
}

fn can_handle(_spec: &Spec) -> bool {
    true
}

#[derive(Clone)]
pub struct LibkrunExecutor {}

impl Executor for LibkrunExecutor {
    fn pre_exec(&self) -> Result<(), ExecutorError> {
        let lib = get_libkrun();
        let krun_create_ctx: Symbol<unsafe extern "C" fn() -> c_int> = unsafe {
            lib.get(b"krun_create_ctx").map_err(|e| {
                ExecutorError::Other(format!("failed to load krun_create_ctx: {}", e))
            })?
        };

        let ctx_id = unsafe { krun_create_ctx() };
        set_ctx_id(ctx_id)?;

        Ok(())
    }

    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        if !can_handle(spec) {
            return Err(ExecutorError::CantHandle(EXECUTOR_NAME));
        }

        let process = spec.process().as_ref();
        let args = process.and_then(|p| p.args().as_ref()).unwrap_or(&EMPTY);
        if args.is_empty() {
            tracing::error!("at least one process arg must be specified");
            return Err(ExecutorError::InvalidArg);
        }
        let cmd = args[0].clone();
        tracing::debug!("process command: {}", cmd);

        unsafe {
            let lib = get_libkrun();
            let ctx_id = get_ctx_id();

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

            krun_set_log_level(1);

            let rc = krun_set_vm_config(ctx_id, 1, 512);
            if rc < 0 {
                return Err(ExecutorError::Other(format!("krun_set_vm_config rc={rc}")));
            }

            let root = CString::new("/").unwrap();
            let rc = krun_set_root(ctx_id, root.as_ptr());
            if rc < 0 {
                return Err(ExecutorError::Other(format!("krun_set_root rc={rc}")));
            }

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
