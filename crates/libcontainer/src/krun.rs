use nix::errno::Errno;
use nix::sys::stat::{major, minor, stat};
use oci_spec::runtime::{LinuxDeviceCgroup, LinuxDeviceCgroupBuilder, LinuxDeviceType, Spec};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum KrunError {
    #[error("{0}")]
    Other(String),
}

type Result<T> = std::result::Result<T, KrunError>;

// executorでやるのはread-onlyになっているので難しい
pub fn write_krun_config(rootfs: &Path, json_spec: &str) -> Result<()> {
    let krun_config_file = ".krun_config.json";
    let config_path = rootfs.join(krun_config_file);
    println!("writing .krun_config.json to: {}", config_path.display());

    // TODO safely
    // https://github.com/containers/crun/blob/main/src/libcrun/handlers/krun.c#L397
    fs::write(&config_path, json_spec)
        .map_err(|e| KrunError::Other(format!("fs::write failed: {}", e)))?;

    Ok(())
}

/// linux device cgroup
pub fn libkrun_modify_spec(spec: &mut Spec) -> Result<()> {
    let Some(linux) = spec.linux() else {
        return Ok(());
    };
    let mut linux = linux.clone();

    let Some(mut res) = linux.resources().clone() else {
        return Ok(());
    };

    let mut devices: Vec<LinuxDeviceCgroup> = res.devices().clone().unwrap_or_default();

    let mut has_kvm = true;

    let (kvm_major, kvm_minor) = match stat_dev_numbers("/dev/kvm") {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            has_kvm = false;
            (0, 0)
        }
        Err(e) => {
            tracing::error!(?e, "failed to stat /dev/kvm");
            return Err(KrunError::Other(format!("stat `/dev/kvm`: {e}")));
        }
    };

    if has_kvm {
        devices.push(make_oci_spec_dev(
            LinuxDeviceType::A,
            kvm_major,
            kvm_minor,
            true,
            "rwm",
        ));
    }

    res.set_devices(Some(devices));
    linux.set_resources(Some(res));
    spec.set_linux(Some(linux));

    Ok(())
}

fn make_oci_spec_dev(
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

fn stat_dev_numbers(path: &str) -> std::io::Result<(i64, i64)> {
    match stat(Path::new(path)) {
        Ok(st) => Ok((major(st.st_rdev) as i64, minor(st.st_rdev) as i64)),
        Err(Errno::ENOENT) => Err(io::Error::new(io::ErrorKind::NotFound, "not found")),
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
    }
}

// use std::path::Path;

// tracing::debug!("mknod kvm");
// use crate::rootfs::device::Device;
// let dev = Device::new();
// use oci_spec::runtime::{LinuxDeviceBuilder, LinuxDeviceType};

// let kvm = LinuxDeviceBuilder::default()
//     .typ(LinuxDeviceType::C)
//     .path(PathBuf::from("/dev/kvm"))
//     .major(10)
//     .minor(232)
//     .file_mode(0o666u32)
//     .uid(0u32)
//     .gid(0u32)
//     .build()
//     .unwrap();

// let _ = dev.create_devices(Path::new("/"), std::slice::from_ref(&kvm), false);
