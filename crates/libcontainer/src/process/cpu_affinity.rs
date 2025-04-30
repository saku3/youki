use std::mem::MaybeUninit;

use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};
use nix::unistd::Pid;

use crate::process::container_intermediate_process::IntermediateProcessError;

pub fn parse_cpuset_string(cpuset: &str) -> Result<Vec<u32>, IntermediateProcessError> {
    let mut cpus = Vec::new();
    for part in cpuset.split(',') {
        if let Some((start, end)) = part.split_once('-') {
            let start: u32 = start
                .parse()
                .map_err(|e| IntermediateProcessError::Other(format!("Invalid number: {}", e)))?;
            let end: u32 = end
                .parse()
                .map_err(|e| IntermediateProcessError::Other(format!("Invalid number: {}", e)))?;
            if start > end {
                return Err(IntermediateProcessError::Other(format!(
                    "Start > End in {}",
                    part
                )));
            }
            for cpu in start..=end {
                cpus.push(cpu);
            }
        } else {
            let cpu: u32 = part
                .parse()
                .map_err(|e| IntermediateProcessError::Other(format!("Invalid CPU: {}", e)))?;
            cpus.push(cpu);
        }
    }
    Ok(cpus)
}

pub fn set_cpuset_affinity(pid: Pid, cpus: Vec<u32>) -> Result<(), IntermediateProcessError> {
    let mut cpuset = MaybeUninit::<cpu_set_t>::uninit();

    unsafe {
        let cpuset_ptr = cpuset.as_mut_ptr();
        CPU_ZERO(&mut *cpuset_ptr);
        for cpu in cpus {
            CPU_SET(cpu as usize, &mut *cpuset_ptr);
        }

        let cpuset = cpuset.assume_init();
        let res = sched_setaffinity(pid.as_raw(), std::mem::size_of::<cpu_set_t>(), &cpuset);
        if res != 0 {
            return Err(IntermediateProcessError::Syscall(
                crate::syscall::SyscallError::IO(std::io::Error::last_os_error()),
            ));
        }
    }

    Ok(())
}
