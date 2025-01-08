mod tests;
mod utils;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use contest::logger;
use test_framework::TestManager;
use tests::cgroups;

use crate::tests::devices::get_devices_test;
use crate::tests::domainname::get_domainname_tests;
use crate::tests::example::get_example_test;
use crate::tests::fd_control::get_fd_control_test;
use crate::tests::hooks::get_hooks_tests;
use crate::tests::hostname::get_hostname_test;
use crate::tests::intel_rdt::get_intel_rdt_test;
use crate::tests::io_priority::get_io_priority_test;
use crate::tests::lifecycle::{ContainerCreate, ContainerLifecycle};
use crate::tests::linux_ns_itype::get_ns_itype_tests;
use crate::tests::mounts_recursive::get_mounts_recursive_test;
use crate::tests::no_pivot::get_no_pivot_test;
use crate::tests::pidfile::get_pidfile_test;
use crate::tests::process::get_process_test;
use crate::tests::process_oom_score_adj::get_process_oom_score_adj_test;
use crate::tests::process_rlimits::get_process_rlimits_test;
use crate::tests::process_user::get_process_user_test;
use crate::tests::readonly_paths::get_ro_paths_test;
use crate::tests::root_readonly_true::get_root_readonly_test;
use crate::tests::rootfs_propagation::get_rootfs_propagation_test;
use crate::tests::scheduler::get_scheduler_test;
use crate::tests::seccomp::get_seccomp_test;
use crate::tests::seccomp_notify::get_seccomp_notify_test;
use crate::tests::sysctl::get_sysctl_test;
use crate::tests::tlb::get_tlb_test;
use crate::utils::support::{set_runtime_path, set_runtimetest_path};

#[derive(Parser, Debug)]
#[clap(version = "0.0.1", author = "youki team")]
struct Opts {
    /// Enables debug output
    #[clap(short, long)]
    debug: bool,

    #[clap(subcommand)]
    command: SubCommand,
}

#[derive(Parser, Debug)]
enum SubCommand {
    /// run the integration tests
    Run(Run),
    /// list available integration tests
    List,
}

#[derive(Parser, Debug)]
struct Run {
    /// Path for the container runtime to be tested
    #[clap(long)]
    runtime: PathBuf,
    /// Path for the runtimetest binary, which will be used to run tests inside the container
    #[clap(long)]
    runtimetest: PathBuf,
    /// Selected tests to be run, format should be
    /// space separated groups, eg
    /// -t group1::test1,test3 group2 group3::test5
    #[clap(short, long, num_args(1..), value_delimiter = ' ')]
    tests: Option<Vec<String>>,
}

// parse test string given in commandline option as pair of testgroup name and tests belonging to that
fn parse_tests(tests: &[String]) -> Vec<(&str, Option<Vec<&str>>)> {
    let mut ret = Vec::with_capacity(tests.len());
    for test in tests {
        if test.contains("::") {
            let (mod_name, test_names) = test.split_once("::").unwrap();
            let _tests = test_names.split(',').collect();
            ret.push((mod_name, Some(_tests)));
        } else {
            ret.push((test, None));
        }
    }
    ret
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    if let Err(e) = logger::init(opts.debug) {
        eprintln!("logger could not be initialized: {e:?}");
    }

    ////////// ANCHOR: register_example_test
    let mut tm = TestManager::new();
    let example = get_example_test();
    tm.add_test_group(Box::new(example));
    ////////// ANCHOR_END: register_example_test

    let _cl = ContainerLifecycle::new();
    let _cc = ContainerCreate::new();
    let _huge_tlb = get_tlb_test();
    let _pidfile = get_pidfile_test();
    let _ns_itype = get_ns_itype_tests();
    let _hooks = get_hooks_tests();
    let _cgroup_v1_pids = cgroups::pids::get_test_group();
    let _cgroup_v1_cpu = cgroups::cpu::v1::get_test_group();
    let _cgroup_v2_cpu = cgroups::cpu::v2::get_test_group();
    let _cgroup_v1_memory = cgroups::memory::get_test_group();
    let _cgroup_v1_network = cgroups::network::get_test_group();
    let _cgroup_v1_blkio = cgroups::blkio::get_test_group();
    let _seccomp = get_seccomp_test();
    let _seccomp_notify = get_seccomp_notify_test();
    let _ro_paths = get_ro_paths_test();
    let _hostname = get_hostname_test();
    let _mounts_recursive = get_mounts_recursive_test();
    let _domainname = get_domainname_tests();
    let _intel_rdt = get_intel_rdt_test();
    let _sysctl = get_sysctl_test();
    let _scheduler = get_scheduler_test();
    let _io_priority_test = get_io_priority_test();
    let _devices = get_devices_test();
    let _root_readonly = get_root_readonly_test();
    let _process = get_process_test();
    let _process_user = get_process_user_test();
    let _process_rlimtis = get_process_rlimits_test();
    let _no_pivot = get_no_pivot_test();
    let _process_oom_score_adj = get_process_oom_score_adj_test();
    let _fd_control = get_fd_control_test();
    let rootfs_propagation = get_rootfs_propagation_test();

    tm.add_test_group(Box::new(rootfs_propagation));

    tm.add_cleanup(Box::new(cgroups::cleanup_v1));
    tm.add_cleanup(Box::new(cgroups::cleanup_v2));

    match opts.command {
        SubCommand::Run(args) => run(args, &tm).context("run tests")?,
        SubCommand::List => list(&tm).context("list tests")?,
    }

    Ok(())
}

fn get_abs_path(rel_path: &Path) -> PathBuf {
    match std::fs::canonicalize(rel_path) {
        // path is relative or resolved correctly
        Ok(path) => path,
        // path is name of program which probably exists in $PATH
        Err(_) => match which::which(rel_path) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error in finding path {rel_path:?} : {e}\nexiting.");
                std::process::exit(66);
            }
        },
    }
}

fn run(opts: Run, test_manager: &TestManager) -> Result<()> {
    let runtime_path = get_abs_path(&opts.runtime);
    set_runtime_path(&runtime_path);

    let runtimetest_path = get_abs_path(&opts.runtimetest);
    set_runtimetest_path(&runtimetest_path);

    if let Some(tests) = opts.tests {
        let tests_to_run = parse_tests(&tests);
        test_manager.run_selected(tests_to_run);
    } else {
        test_manager.run_all();
    }

    Ok(())
}

fn list(test_manager: &TestManager) -> Result<()> {
    for test_group in test_manager.tests_groups() {
        println!("{test_group}");
    }

    Ok(())
}
