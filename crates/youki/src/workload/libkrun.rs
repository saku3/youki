use anyhow::anyhow;
use libcontainer::oci_spec::runtime::Spec;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError, EMPTY};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use std::ffi::CString;
use libloading::{Library, Symbol};
use std::os::raw::{c_char, c_int};
use once_cell::sync::Lazy;

const EXECUTOR_NAME: &str = "libkrun";
const LIBKRUN_NAME: &str = "libkrun.so.1";

// Lazy loading of libkrun
static LIBKRUN: Lazy<Option<Arc<Library>>> = Lazy::new(|| {
    unsafe { Library::new(LIBKRUN_NAME).ok().map(Arc::new) }
});

// Lazy, mutable ctx_id holder
static CTX_ID: Lazy<Mutex<Option<c_int>>> = Lazy::new(|| Mutex::new(None));

fn get_libkrun() -> Arc<Library> {
    LIBKRUN
        .as_ref()
        .expect("libkrun not preloaded")
        .clone()
}

fn set_ctx_id(value: c_int) -> Result<(), ExecutorError> {
    let mut guard = CTX_ID.lock().unwrap();
    if guard.is_some() {
        return Err(ExecutorError::Other("ctx_id already initialized".to_string()));
    }
    *guard = Some(value);
    Ok(())
}

fn get_ctx_id() -> c_int {
    let guard = CTX_ID.lock().unwrap();
    guard
        .expect("ctx_id not initialized. Call pre_exec() first.")
}

fn can_handle(_spec: &Spec) -> bool {
    true
}

fn modify_oci_configuration() {
    println!("modify_oci_configuration");
}

#[derive(Clone)]
pub struct LibkrunExecutor {}

impl Executor for LibkrunExecutor {
    fn pre_exec(&self, spec: &mut Spec) -> Result<(), ExecutorError> {

        let json_spec = serde_json::to_string_pretty(&spec).map_err(|e| {
            ExecutorError::Other(format!("failed to serialize spec to JSON: {}", e))
        })?;
        println!("pre_exec: spec as JSON:\n{}", json_spec);

        let lib = get_libkrun();
        let krun_create_ctx: Symbol<unsafe extern "C" fn() -> c_int> = unsafe {
            lib.get(b"krun_create_ctx").map_err(|e| {
                ExecutorError::Other(format!("failed to load krun_create_ctx: {}", e))
            })?
        };

        let ctx_id = unsafe { krun_create_ctx() };
        set_ctx_id(ctx_id)?;

        modify_oci_configuration();

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

        let process = spec.process().as_ref();
        let args = process
            .and_then(|p| p.args().as_ref())
            .unwrap_or(&EMPTY);
        if args.is_empty() {
            tracing::error!("at least one process arg must be specified");
            return Err(ExecutorError::InvalidArg);
        }

        let mut cmd = args[0].clone();
        if let Some(stripped) = args[0].strip_prefix(std::path::MAIN_SEPARATOR) {
            cmd = stripped.to_string();
            tracing::debug!("process command: {}", cmd);
        }

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

            let krun_set_exec: Symbol<
                unsafe extern "C" fn(c_int, *const c_char, c_int, *const *const c_char) -> c_int,
            > = lib.get(b"krun_set_exec").map_err(|e| {
                ExecutorError::Other(format!("failed to load krun_set_exec: {}", e))
            })?;

            let krun_start_enter: Symbol<unsafe extern "C" fn(c_int) -> c_int> =
                lib.get(b"krun_start_enter").map_err(|e| {
                    ExecutorError::Other(format!("failed to load krun_start_enter: {}", e))
                })?;

            krun_set_vm_config(ctx_id, 1, 512);

            let root = CString::new("/").unwrap();
            krun_set_root(ctx_id, root.as_ptr());

            let bin = CString::new(cmd).unwrap();
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


// use anyhow::{Context, Result};
// use nix::errno::Errno;
// use nix::sys::stat::stat;
// use oci_spec::runtime::{
//     LinuxDeviceCgroup, LinuxDeviceCgroupBuilder, LinuxDeviceType, LinuxResourcesBuilder, Spec,
// };

// /// /dev/kvm がホストに存在する場合に、コンテナからの rwm アクセスを許可する
// pub fn allow_dev_kvm(spec: &mut Spec) -> Result<()> {
//     // 1) stat(/dev/kvm) で存在確認
//     let st = match stat("/dev/kvm") {
//         Ok(st) => st,
//         Err(nix::Error::Sys(Errno::ENOENT)) => {
//             // デバイスが無い環境では何もしない
//             return Ok(());
//         }
//         Err(e) => {
//             // それ以外の stat エラーは原因を出して返す
//             return Err(anyhow::anyhow!(e)).context("stat(/dev/kvm) failed");
//         }
//     };

//     // 2) major/minor を取得
//     //   libc::major/minor は非安全関数なのでラップ
//     let major = unsafe { libc::major(st.st_rdev) as i64 };
//     let minor = unsafe { libc::minor(st.st_rdev) as i64 };

//     // 3) allow エントリを生成（type = 'a', access = "rwm"）
//     let kvm_rule: LinuxDeviceCgroup = LinuxDeviceCgroupBuilder::default()
//         .allow(true)
//         .typ(LinuxDeviceType::A)
//         .major(major)
//         .minor(minor)
//         .access("rwm".to_string())
//         .build()
//         .context("build LinuxDeviceCgroup for /dev/kvm")?;

//     // 4) spec.linux.resources.devices に追記
//     let linux = spec
//         .linux_mut()
//         .context("spec.linux is None (no linux section in spec)")?;

//     // resources が無ければ作る
//     if linux.resources().is_none() {
//         linux.set_resources(
//             LinuxResourcesBuilder::default()
//                 .devices(Vec::new())
//                 .build()
//                 .context("create empty LinuxResources")?,
//         );
//     }

//     // devices ベクタを取得して push
//     let resources = linux.resources_mut().expect("just set above");
//     let devices = resources.devices_mut().get_or_insert_with(Vec::new);

//     // 既存重複（同一 type/major/minor/allow/access）を簡易スキップ
//     let is_dup = devices.iter().any(|d| {
//         d.allow() == Some(true)
//             && d.typ() == Some(LinuxDeviceType::A)
//             && d.major() == Some(major)
//             && d.minor() == Some(minor)
//             && d.access().as_deref() == Some("rwm")
//     });
//     if !is_dup {
//         devices.push(kvm_rule);
//     }

//     Ok(())
// }
