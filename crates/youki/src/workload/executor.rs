use libcontainer::oci_spec::runtime::Spec;
use libcontainer::workload::{Executor, ExecutorError, ExecutorValidationError};

#[derive(Clone)]
pub struct DefaultExecutor {}

impl Executor for DefaultExecutor {
    fn pre_exec(&self, spec: Spec) -> Result<Spec, ExecutorError> {
        #[cfg(feature = "libkrun")]
        {
            tracing::debug!("trying libkrun pre executor");
            match super::libkrun::get_executor().pre_exec(spec) {
                Ok(spec_pre_exec) => {
                    tracing::debug!("libkrun executor accepted workload");
                    Ok(spec_pre_exec)
                }
                Err(ExecutorError::CantHandle(e)) => {
                    tracing::debug!("libkrun executor cannot handle this spec: {}", e);
                    Err(ExecutorError::CantHandle(e))
                }
                Err(err) => {
                    tracing::error!("libkrun executor failed: {:?}", err);
                    Err(err)
                }
            }
        }

        #[cfg(not(feature = "libkrun"))]
        {
            tracing::debug!("libkrun feature is not enabled; skipping pre_exec");
            Ok(spec)
        }
    }

    fn exec(&self, spec: &Spec) -> Result<(), ExecutorError> {
        tracing::debug!("executing libkrun executer");
        #[cfg(feature = "wasm-wasmer")]
        match super::wasmer::get_executor().exec(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        #[cfg(feature = "wasm-wasmedge")]
        match super::wasmedge::get_executor().exec(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        #[cfg(feature = "wasm-wasmtime")]
        match super::wasmtime::get_executor().exec(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        #[cfg(feature = "libkrun")]
        {
            tracing::debug!("trying libkrun executor");
            match super::libkrun::get_executor().exec(spec) {
                Ok(_) => {
                    tracing::debug!("libkrun executor accepted workload");
                    return Ok(());
                }
                Err(ExecutorError::CantHandle(_)) => {
                    tracing::debug!("libkrun executor cannot handle this spec");
                }
                Err(err) => {
                    tracing::error!("libkrun executor failed: {:?}", err);
                    return Err(err);
                }
            }
        }

        // Leave the default executor as the last option, which executes normal
        // container workloads.
        libcontainer::workload::default::get_executor().exec(spec)
    }

    fn validate(&self, spec: &Spec) -> Result<(), ExecutorValidationError> {
        tracing::debug!("validate libkrun executer");
        #[cfg(feature = "wasm-wasmer")]
        match super::wasmer::get_executor().validate(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorValidationError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        #[cfg(feature = "wasm-wasmedge")]
        match super::wasmedge::get_executor().validate(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorValidationError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        #[cfg(feature = "wasm-wasmtime")]
        match super::wasmtime::get_executor().validate(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorValidationError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        #[cfg(feature = "libkrun")]
        match super::libkrun::get_executor().validate(spec) {
            Ok(_) => return Ok(()),
            Err(ExecutorValidationError::CantHandle(_)) => (),
            Err(err) => return Err(err),
        }
        libcontainer::workload::default::get_executor().validate(spec)
    }
}

pub fn default_executor() -> DefaultExecutor {
    DefaultExecutor {}
}
