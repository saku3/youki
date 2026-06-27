use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use oci_spec::runtime::{
    LinuxBuilder, LinuxDeviceBuilder, LinuxDeviceCgroupBuilder, LinuxDeviceType,
    LinuxResourcesBuilder, ProcessBuilder, Spec, SpecBuilder,
};
use test_framework::{ConditionalTest, TestGroup, TestResult, test_result};

use crate::utils::test_inside_container;
use crate::utils::test_utils::CreateOptions;

const DENIED_DEVICE_PATH: &str = "/dev/denied-access";
const DENIED_MAJOR: i64 = 511;
const DENIED_MINOR: i64 = 0;

fn create_spec() -> Result<Spec> {
    let cgroups_path = PathBuf::from("system.slice:youki_contest:devices");

    let denied_node = LinuxDeviceBuilder::default()
        .path(PathBuf::from(DENIED_DEVICE_PATH))
        .typ(LinuxDeviceType::C)
        .major(DENIED_MAJOR)
        .minor(DENIED_MINOR)
        .file_mode(0o666u32)
        .build()
        .context("failed to build denied device node")?;

    let deny_rule = LinuxDeviceCgroupBuilder::default()
        .allow(false)
        .typ(LinuxDeviceType::C)
        .major(DENIED_MAJOR)
        .minor(DENIED_MINOR)
        .access("rwm")
        .build()
        .context("failed to build deny device rule")?;

    let spec = SpecBuilder::default()
        .process(
            ProcessBuilder::default()
                .args(vec![
                    "runtimetest".to_string(),
                    "device_cgroup_denied".to_string(),
                ])
                .build()
                .context("failed to build process config")?,
        )
        .linux(
            LinuxBuilder::default()
                .cgroups_path(cgroups_path)
                .devices(vec![denied_node])
                .resources(
                    LinuxResourcesBuilder::default()
                        .devices(vec![deny_rule])
                        .build()
                        .context("failed to build resources spec")?,
                )
                .build()
                .context("failed to build linux spec")?,
        )
        .build()
        .context("failed to build spec")?;

    Ok(spec)
}

fn systemd_device_deny_test() -> TestResult {
    let spec = test_result!(create_spec());
    test_inside_container(&spec, &CreateOptions::default().with_systemd(), &|_| Ok(()))
}

fn can_run() -> bool {
    Path::new("/sys/fs/cgroup/cgroup.controllers").exists()
        && Path::new("/run/systemd/system").exists()
}

pub fn get_test_group() -> TestGroup {
    let mut test_group = TestGroup::new("cgroup_v2_devices_systemd");
    let deny = ConditionalTest::new(
        "systemd_device_deny",
        Box::new(can_run),
        Box::new(systemd_device_deny_test),
    );
    test_group.add(vec![Box::new(deny)]);
    test_group
}
