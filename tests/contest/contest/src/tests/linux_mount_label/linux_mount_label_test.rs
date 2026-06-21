use oci_spec::runtime::{
    LinuxBuilder, MountBuilder, ProcessBuilder, Spec, SpecBuilder, get_default_mounts,
};
use test_framework::{Test, TestGroup, TestResult};

use crate::utils::test_inside_container;
use crate::utils::test_utils::CreateOptions;

// Destination of the extra mount we add so the in-container runtimetest can
// confirm the labeled mount was set up. Keep in sync with `validate_linux_mount_label`.
const MOUNT_LABEL_TEST_PATH: &str = "/tmp/.tmp";

fn create_spec(linux_mount_label: String) -> Spec {
    // Start from the default mounts and add a tmpfs the mount label applies to,
    // so the container has a mount we can look for from inside.
    let mut mounts = get_default_mounts();
    mounts.push(
        MountBuilder::default()
            .destination(MOUNT_LABEL_TEST_PATH)
            .typ("tmpfs")
            .source("tmpfs")
            .options(vec![
                "nosuid".to_string(),
                "strictatime".to_string(),
                "mode=755".to_string(),
                "size=35m".to_string(),
            ])
            .build()
            .expect("error in building mount"),
    );

    SpecBuilder::default()
        .mounts(mounts)
        .linux(
            // Need to reset the read-only paths
            LinuxBuilder::default()
                .mount_label(linux_mount_label)
                .masked_paths(vec![])
                .build()
                .expect("error in building linux config"),
        )
        .process(
            ProcessBuilder::default()
                .args(vec![
                    "runtimetest".to_string(),
                    "linux_mount_label".to_string(),
                ])
                .build()
                .expect("error in creating process config"),
        )
        .build()
        .unwrap()
}

fn test_linux_mount_label() -> TestResult {
    // The `context=` mount option used to apply the mount label is an SELinux
    // mount option, which the kernel rejects with EINVAL when SELinux is not
    // enabled. Skip the test on hosts without SELinux.
    if !std::path::Path::new("/sys/fs/selinux").exists() {
        return TestResult::Skipped(
            "SELinux is not enabled on this host; skipping linux_mount_label test".to_string(),
        );
    }

    let spec = create_spec("system_u:object_r:svirt_sandbox_file_t:s0:c715,c811".to_string());
    test_inside_container(&spec, &CreateOptions::default(), &|_| {
        // As long as the container is created, we expect the mount label to be determined
        // by the spec, so nothing to prepare prior.
        Ok(())
    })
}

pub fn get_linux_mount_label_test() -> TestGroup {
    let linux_mount_label = Test::new("linux_mount_label", Box::new(test_linux_mount_label));
    let mut tg = TestGroup::new("linux_mount_label");
    tg.add(vec![Box::new(linux_mount_label)]);
    tg
}
