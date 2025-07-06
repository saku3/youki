use std::ffi::{CString, c_void};
use std::ptr;
use std::os::raw::c_char;
use libloading::{Library, Symbol};
use thiserror::Error;

pub struct KrunConfig {
    handle: Option<Library>,
    handle_sev: Option<Library>,
    sev: bool,
    ctx_id: Option<i32>,
    ctx_id_sev: Option<i32>,
}

#[derive(Debug, Error)]
pub enum LibKrunError {
    #[error("failed to load symbol: {0}")]
    SymbolError(String),
    #[error("krun_create_ctx failed with code {0}")]
    CreateContextError(i32),
    #[error("dlopen or symbol resolution failed: {0}")]
    LibraryError(#[from] libloading::Error),
}

impl KrunConfig {
    pub fn load() -> Result<*mut Self, String> {
        let libkrun_so = "libkrun.so.1";
        let libkrun_sev_so = "libkrun-sev.so.1";

        let handle = unsafe { Library::new(libkrun_so).ok() };
        let handle_sev = unsafe { Library::new(libkrun_sev_so).ok() };

        if handle.is_none() && handle_sev.is_none() {
            return Err(format!(
                "failed to open `{}` and `{}` for krun_config",
                libkrun_so, libkrun_sev_so
            ));
        }

        let mut kconf = Box::new(KrunConfig {
            handle,
            handle_sev,
            sev: false,
            ctx_id: None,
            ctx_id_sev: None,
        });

        if let Some(ref lib) = kconf.handle {
            let ctx_id = Self::libkrun_create_context(lib).map_err(|e| e.to_string())?;
            kconf.ctx_id = Some(ctx_id);
        }

        if let Some(ref lib) = kconf.handle_sev {
                let ctx_id = Self::libkrun_create_context(lib).map_err(|e| e.to_string())?;
                kconf.ctx_id_sev = Some(ctx_id);
        }

        Ok(Box::into_raw(kconf))
    }

    pub fn libkrun_create_context(lib: &Library) -> Result<i32, LibKrunError> {
        unsafe {
            let krun_create_ctx: Symbol<unsafe extern "C" fn() -> i32> =
                lib.get(b"krun_create_ctx").map_err(|e| LibKrunError::SymbolError(e.to_string()))?;

            let ctx_id = krun_create_ctx();
            if ctx_id < 0 {
                return Err(LibKrunError::CreateContextError(-ctx_id));
            }

            Ok(ctx_id)
        }
    }
}
