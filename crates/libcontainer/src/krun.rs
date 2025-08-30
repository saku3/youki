// use std::path::{Path, PathBuf};
// use std::{fs, io};

// use nix::errno::Errno;
// use nix::sys::stat::{major, minor, stat};
// use oci_spec::runtime::{
//     LinuxBuilder, LinuxDevice, LinuxDeviceBuilder, LinuxDeviceCgroup, LinuxDeviceCgroupBuilder,
//     LinuxDeviceType, Spec,
// };

// use crate::error::MissingSpecError;

// #[derive(Debug, thiserror::Error)]
// pub enum KrunError {
//     #[error("{0}")]
//     Other(String),
// }

// type Result<T> = std::result::Result<T, KrunError>;

// // add /dev/kvm to linux.device
// pub fn libkrun_modify_spec_device(spec: &mut Spec) -> Result<()> {
//     let mut linux = match spec.linux().clone() {
//         Some(l) => l,
//         None => LinuxBuilder::default()
//             .build()
//             .map_err(|e| KrunError::Other(format!("build default linux section: {e}")))?,
//     };

//     let (kvm_major, kvm_minor) = match stat_dev_numbers("/dev/kvm") {
//         Ok(v) => v,
//         Err(e) if e.kind() == io::ErrorKind::NotFound => {
//             spec.set_linux(Some(linux));
//             return Ok(());
//         }
//         Err(e) => return Err(KrunError::Other(format!("stat `/dev/kvm`: {e}"))),
//     };

//     let mut devices: Vec<LinuxDevice> = linux.devices().clone().unwrap_or_default();

//     let exists = devices.iter().any(|d| d.path() == Path::new("/dev/kvm"));
//     if !exists {
//         devices.push(make_oci_spec_device(
//             PathBuf::from("/dev/kvm"),
//             LinuxDeviceType::C,
//             kvm_major,
//             kvm_minor,
//             0o666u32,
//             0u32,
//             0u32
//         ));
//         linux.set_devices(Some(devices));
//     }

//     spec.set_linux(Some(linux));
//     Ok(())
// }

// // Add an allow rule for /dev/kvm to linux.resources.devices
// // if resources.devices is None or empty, it's effectively permissive, so skip.
// pub fn libkrun_modify_spec_resource_device(spec: &mut Spec) -> Result<()> {
//     let mut linux = match spec.linux() {
//         Some(l) => l.clone(),
//         None => return Ok(()),
//     };
//     let mut res = match linux.resources() {
//         Some(r) => r.clone(),
//         None => return Ok(()),
//     };
//     let mut device_cgroups: Vec<LinuxDeviceCgroup> = match res.devices() {
//         Some(v) if !v.is_empty() => v.clone(),
//         _ => return Ok(()),
//     };

//     let (kvm_major, kvm_minor) = match stat_dev_numbers("/dev/kvm") {
//         Ok(v) => v,
//         Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
//         Err(e) => return Err(KrunError::Other(format!("stat `/dev/kvm`: {e}"))),
//     };

//     device_cgroups.push(make_oci_spec_dev_cgroup(
//         LinuxDeviceType::C,
//         kvm_major,
//         kvm_minor,
//         true,
//         "rwm",
//     ));
//     res.set_devices(Some(device_cgroups));
//     linux.set_resources(Some(res));
//     spec.set_linux(Some(linux));

//     Ok(())
// }

// fn stat_dev_numbers(path: &str) -> std::io::Result<(i64, i64)> {
//     match stat(Path::new(path)) {
//         Ok(st) => Ok((major(st.st_rdev) as i64, minor(st.st_rdev) as i64)),
//         Err(Errno::ENOENT) => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
//         Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
//     }
// }

// pub fn write_krun_config(rootfs: &Path, json_spec: &str) -> Result<()> {
//     let krun_config_file = ".krun_config.json";
//     let config_path = rootfs.join(krun_config_file);
//     println!("writing .krun_config.json to: {}", config_path.display());

//     // TODO atomic write
//     // https://github.com/containers/crun/blob/main/src/libcrun/handlers/krun.c#L397
//     fs::write(&config_path, json_spec)
//         .map_err(|e| KrunError::Other(format!("fs::write failed: {}", e)))?;

//     Ok(())
// }

// pub fn configure_for_libkrun(mut spec: Spec) -> Result<Spec> {
//     let use_krun = {
//         spec.annotations()
//             .as_ref()
//             .and_then(|a| a.get("run.oci.handler"))
//             .map(|v| v == "krun")
//             .unwrap_or(false)
//     };

//     if !use_krun {
//         return Ok(spec);
//     }

//     let rootfs = spec
//         .root()
//         .as_ref()
//         .ok_or(MissingSpecError::Root)
//         .map_err(|e| KrunError::Other(format!("Missing root in spec: {e:?}")))?
//         .path()
//         .to_path_buf();
//     libkrun_modify_spec_device(&mut spec)
//         .map_err(|e| KrunError::Other(format!("libkrun_modify_spec_device: {e}")))?;
//     libkrun_modify_spec_resource_device(&mut  spec)
//         .map_err(|e| KrunError::Other(format!("libkrun_modify_spec: {e}")))?;

//     let json_spec = serde_json::to_string_pretty(&spec)
//         .map_err(|e| KrunError::Other(format!("failed to serialize spec to JSON: {}", e)))?;
//     write_krun_config(&rootfs, &json_spec)
//         .map_err(|e| KrunError::Other(format!("write_krun_config: {e}")))?;
//     Ok(spec)
// }

// fn make_oci_spec_dev_cgroup(
//     dev_type: LinuxDeviceType,
//     major_num: i64,
//     minor_num: i64,
//     allow: bool,
//     access: &str,
// ) -> LinuxDeviceCgroup {
//     LinuxDeviceCgroupBuilder::default()
//         .allow(allow)
//         .typ(dev_type)
//         .major(major_num)
//         .minor(minor_num)
//         .access(access.to_string())
//         .build()
//         .expect("device cgroup build")
// }

// fn make_oci_spec_device(
//     path: impl Into<PathBuf>,
//     dev_type: LinuxDeviceType,
//     major_num: i64,
//     minor_num: i64,
//     file_mode: u32,
//     uid: u32,
//     gid: u32,
// ) -> LinuxDevice{
//     LinuxDeviceBuilder::default()
//         .typ(dev_type)
//         .path(path.into())
//         .major(major_num)
//         .minor(minor_num)
//         .file_mode(file_mode)
//         .uid(uid)
//         .gid(gid)
//         .build()
//         .expect("device node build")
//         // .map_err(|e| KrunError::Other(format!("build linux.devices node: {e}")))
// }
