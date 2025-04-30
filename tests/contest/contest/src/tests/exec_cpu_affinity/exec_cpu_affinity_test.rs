use anyhow::{anyhow, Context, Result};
use oci_spec::runtime::{ExecCPUAffinityBuilder, ProcessBuilder, Spec, SpecBuilder};
use test_framework::{test_result, Test, TestGroup, TestResult};

use crate::utils::{exec_in_container, start_container, test_outside_container};

fn create_spec() -> Result<Spec> {
    let cpu_affinity = ExecCPUAffinityBuilder::default()
        .initial("0-1".to_string())
        .cpu_affinity_final("0-1".to_string())
        .build()?;

    SpecBuilder::default()
        .process(
            ProcessBuilder::default()
                .args(vec!["sleep".to_string(), "30000".to_string()])
                .exec_cpu_affinity(cpu_affinity)
                .build()?,
        )
        .build()
        .context("failed to create spec")
}

fn exec_cpu_affinity_test() -> TestResult {
    let spec = test_result!(create_spec());
    test_outside_container(&spec, &|data| {
        let id = &data.id;
        let dir = &data.bundle;

        let start_result = start_container(id, dir).unwrap().wait().unwrap();
        if !start_result.success() {
            return TestResult::Failed(anyhow!("container start failed"));
        }

        let (stdout, stderr) =
            exec_in_container(id, dir, &["grep", "Cpus_allowed_list", "/proc/self/status"])
                .expect("exec failed");

        println!("stdout: {}", stdout);
        println!("stderr: {}", stderr);

        if !stdout.contains("0-1") {
            return TestResult::Failed(anyhow!("unexpected Cpus_allowed_list: {}", stdout));
        }

        TestResult::Passed
    })
}

pub fn get_exec_cpu_affinity_test() -> TestGroup {
    let mut exec_cpu_affinity_test_group = TestGroup::new("exec_cpu_affinity");

    let test = Test::new("exec_cpu_affinity_test", Box::new(exec_cpu_affinity_test));
    exec_cpu_affinity_test_group.add(vec![Box::new(test)]);

    exec_cpu_affinity_test_group
}
