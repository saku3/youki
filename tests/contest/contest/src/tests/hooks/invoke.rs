use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

use anyhow::anyhow;
use oci_spec::runtime::{Hook, HookBuilder, HooksBuilder, ProcessBuilder, Spec, SpecBuilder};
use test_framework::{Test, TestGroup, TestResult};

use crate::utils::test_utils::{CreateOptions, start_container};
use crate::utils::{create_container, delete_container, generate_uuid, prepare_bundle, set_config};

const HOOK_OUTPUT_FILE: &str = "output";

fn create_hook_output_file() {
    std::fs::File::create(HOOK_OUTPUT_FILE).expect("fail to create hook output file");
}

fn delete_hook_output_file() {
    std::fs::remove_file(HOOK_OUTPUT_FILE).expect("fail to remove hook output file");
}

fn append_log(line: &str) {
    let p = std::fs::canonicalize(HOOK_OUTPUT_FILE).expect("canonicalize output");
    let mut f = OpenOptions::new()
        .append(true)
        .open(p)
        .expect("open for append");
    writeln!(f, "{}", line).expect("append log");
}

fn write_log_hook(content: &str) -> Hook {
    let output = std::fs::canonicalize(HOOK_OUTPUT_FILE).unwrap();
    let output = output.to_str().unwrap();
    HookBuilder::default()
        .path("/bin/sh")
        .args(vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("echo '{content}' >> {output}",),
        ])
        .build()
        .expect("could not build hook")
}

fn get_spec() -> Spec {
    SpecBuilder::default()
        .process(
            ProcessBuilder::default()
                .args(vec!["true".to_string()])
                .build()
                .unwrap(),
        )
        .hooks(
            HooksBuilder::default()
                .prestart(vec![
                    write_log_hook("prestart-1 called"),
                    write_log_hook("prestart-2 called"),
                ])
                .create_runtime(vec![
                    write_log_hook("createRuntime-1 called"),
                    write_log_hook("createRuntime-2 called"),
                ])
                .create_container(vec![
                    write_log_hook("createContainer-1 called"),
                    write_log_hook("createContainer-2 called"),
                ])
                .start_container(vec![
                    write_log_hook("startContainer-1 called"),
                    write_log_hook("startContainer-2 called"),
                ])
                .poststart(vec![
                    write_log_hook("poststart-1 called"),
                    write_log_hook("poststart-2 called"),
                ])
                .poststop(vec![
                    write_log_hook("poststop-1 called"),
                    write_log_hook("poststop-2 called"),
                ])
                .build()
                .expect("could not build hooks"),
        )
        .build()
        .unwrap()
}

fn get_test(test_name: &'static str) -> Test {
    Test::new(
        test_name,
        Box::new(move || {
            create_hook_output_file();
            let spec = get_spec();
            let id = generate_uuid();
            let id_str = id.to_string();
            let bundle = prepare_bundle().unwrap();
            set_config(&bundle, &spec).unwrap();

            append_log("before_create");
            create_container(&id_str, &bundle, &CreateOptions::default())
                .unwrap()
                .wait()
                .unwrap();
            append_log("after_create");

            append_log("before_start");
            start_container(&id_str, &bundle).unwrap().wait().unwrap();
            append_log("after_start");

            append_log("before_delete");
            delete_container(&id_str, &bundle).unwrap().wait().unwrap();
            append_log("after_delete");

            let log = {
                let mut output = File::open("output").expect("cannot open hook log");
                let mut log = String::new();
                output
                    .read_to_string(&mut log)
                    .expect("fail to read hook log");
                log
            };
            delete_hook_output_file();

            let expected = "before_create\n\
                    prestart-1 called\n\
                    prestart-2 called\n\
                    createRuntime-1 called\n\
                    createRuntime-2 called\n\
                    createContainer-1 called\n\
                    createContainer-2 called\n\
                    poststart-1 called\n\
                    poststart-2 called\n\
                    after_create\n\
                    before_start\n\
                    after_start\n\
                    before_delete\n\
                    poststop-1 called\n\
                    poststop-2 called\n\
                    after_delete\n";
            if log != expected {
                return TestResult::Failed(anyhow!(
                    "error: hooks must be called in the listed order.\n--- got ---\n{log}\n--- expected ---\n{expected}"
                ));
            }
            TestResult::Passed
        }),
    )
}

pub fn get_hooks_tests() -> TestGroup {
    let mut tg = TestGroup::new("hooks");
    tg.add(vec![Box::new(get_test("hooks"))]);
    tg
}
