pub mod apparmor;
pub mod capabilities;
pub mod channel;
pub mod config;
pub mod container;
pub mod error;
pub mod hooks;
pub mod namespaces;
pub mod network;
pub mod notify_socket;
pub mod process;
pub mod rootfs;
#[cfg(feature = "libseccomp")]
pub mod seccomp;
pub mod signal;
pub mod syscall;
pub mod test_utils;
pub mod tty;
pub mod user_ns;
pub mod utils;

#[cfg(all(feature = "cgroupsv2_devices", feature = "libseccomp"))]
compile_error!("[feature test] intentional build failure: cgroupsv2_devices + libseccomp");

#[cfg(all(test, feature = "v1"))]
#[test]
fn run_intentional_feature_test_failure() {
    panic!("[feature test] intentional test failure under feature v1");
}
pub mod validator;
pub mod workload;

// Because the `libcontainer` api uses the oci_spec who resides in a different
// crate, we re-export the version of oci_spec this crate uses.
// Ref: https://github.com/youki-dev/youki/issues/2066
// Ref: https://github.com/rust-lang/api-guidelines/discussions/176
pub use oci_spec;
