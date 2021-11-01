use std::collections::HashMap;

use anyhow::{Context, Result};
use dbus::arg::RefArg;
use oci_spec::runtime::LinuxPids;

use crate::common::ControllerOpt;

use super::controller::Controller;

pub struct Pids {}

impl Controller for Pids {
    fn apply(
        options: &ControllerOpt,
        _: u32,
        properties: &mut HashMap<String, Box<dyn RefArg>>,
    ) -> Result<()> {
        if let Some(pids) = options.resources.pids() {
            log::debug!("Applying pids resource restrictions");
            return Self::apply(pids, properties).context("");
        }

        Ok(())
    }
}

impl Pids {
    fn apply(pids: &LinuxPids, properties: &mut HashMap<String, Box<dyn RefArg>>) -> Result<()> {
        let limit = if pids.limit() > 0 {
            pids.limit() as u64
        } else {
            u64::MAX
        };

        properties.insert("TasksMax".to_owned(), Box::new(limit));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbus::arg::ArgType;
    use oci_spec::runtime::{LinuxPidsBuilder, LinuxResources, LinuxResourcesBuilder};

    fn setup(resources: &LinuxResources) -> (ControllerOpt, HashMap<String, Box<dyn RefArg>>) {
        let properties = HashMap::new();
        let options = ControllerOpt {
            resources: &resources,
            disable_oom_killer: false,
            oom_score_adj: None,
            freezer_state: None,
        };

        (options, properties)
    }

    #[test]
    fn test_pids_positive_limit() -> Result<()> {
        let resources = LinuxResourcesBuilder::default()
            .pids(LinuxPidsBuilder::default().limit(10).build()?)
            .build()?;
        let (options, mut properties) = setup(&resources);

        <Pids as Controller>::apply(&options, 245, &mut properties).context("apply pids")?;

        assert_eq!(properties.len(), 1);
        assert!(properties.contains_key("TasksMax"));

        let task_max = properties.get("TasksMax").unwrap();
        assert_eq!(task_max.arg_type(), ArgType::UInt64);
        assert_eq!(task_max.as_u64().unwrap(), 10);

        Ok(())
    }

    #[test]
    fn test_pids_zero_limit() -> Result<()> {
        let resources = LinuxResourcesBuilder::default()
            .pids(LinuxPidsBuilder::default().limit(0).build()?)
            .build()?;
        let (options, mut properties) = setup(&resources);

        <Pids as Controller>::apply(&options, 245, &mut properties).context("apply pids")?;

        assert_eq!(properties.len(), 1);
        assert!(properties.contains_key("TasksMax"));

        let task_max = properties.get("TasksMax").unwrap();
        assert_eq!(task_max.arg_type(), ArgType::UInt64);
        assert_eq!(task_max.as_u64().unwrap(), u64::MAX);

        Ok(())
    }

    #[test]
    fn test_pids_negative_limit() -> Result<()> {
        let resources = LinuxResourcesBuilder::default()
            .pids(LinuxPidsBuilder::default().limit(-500).build()?)
            .build()?;
        let (options, mut properties) = setup(&resources);

        <Pids as Controller>::apply(&options, 245, &mut properties).context("apply pids")?;

        assert_eq!(properties.len(), 1);
        assert!(properties.contains_key("TasksMax"));

        let task_max = properties.get("TasksMax").unwrap();
        assert_eq!(task_max.arg_type(), ArgType::UInt64);
        assert_eq!(task_max.as_u64().unwrap(), u64::MAX);

        Ok(())
    }
}
