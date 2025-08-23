use std::fs;
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

    // TODO
    // 安全にやる必要がある
    // https://github.com/containers/crun/blob/main/src/libcrun/handlers/krun.c#L397
    fs::write(&config_path, json_spec)
        .map_err(|e| KrunError::Other(format!("fs::write failed: {}", e)))?;

    Ok(())
}
