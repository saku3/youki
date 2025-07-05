use std::ffi::{CString, c_void};
use std::ptr;
use std::os::raw::c_char;
use libloading::{Library, Symbol};

pub struct KrunConfig {
    handle: Option<Library>,
    handle_sev: Option<Library>,
    sev: bool,
    ctx_id: Option<i32>,
    ctx_id_sev: Option<i32>,
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
            unsafe {
                let krun_create_ctx: Symbol<unsafe extern "C" fn() -> i32> =
                    lib.get(b"krun_create_ctx").map_err(|e| e.to_string())?;
                let ret = krun_create_ctx();
                if ret < 0 {
                    return Err("krun_create_ctx failed".into());
                }
                kconf.ctx_id = Some(ret);
            }
        }

        if let Some(ref lib) = kconf.handle_sev {
            unsafe {
                let krun_create_ctx: Symbol<unsafe extern "C" fn() -> i32> =
                    lib.get(b"krun_create_ctx").map_err(|e| e.to_string())?;
                let ret = krun_create_ctx();
                if ret < 0 {
                    return Err("krun_create_ctx (SEV) failed".into());
                }
                kconf.ctx_id_sev = Some(ret);
            }
        }

        Ok(Box::into_raw(kconf))
    }
}
