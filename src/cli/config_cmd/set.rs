use std::path::Path;

use anyhow::{Context, Result};
use serde_yaml::Value;

use super::dotted_path;

pub fn run(config_path: &Path, key: &str, value: &str) -> Result<()> {
    let raw = std::fs::read_to_string(config_path)
        .with_context(|| format!("cannot read config file: {}", config_path.display()))?;

    let mut yaml: Value = serde_yaml::from_str(&raw).context("config file is not valid YAML")?;

    let segments = dotted_path::parse(key)?;
    let target = dotted_path::resolve_mut(&mut yaml, &segments)?;
    *target = dotted_path::auto_value(value);

    let new_yaml = serde_yaml::to_string(&yaml).context("failed to serialize config")?;

    // Validate by loading through the full config pipeline before writing.
    // Write to a temp file so load_from_path can read it.
    let temp_path = config_path.with_extension("yaml.tmp");
    std::fs::write(&temp_path, &new_yaml)
        .with_context(|| format!("cannot write temp file: {}", temp_path.display()))?;

    let validation = crate::config::load_from_path(&temp_path);
    let _ = std::fs::remove_file(&temp_path);

    if let Err(e) = validation {
        anyhow::bail!("change rejected — config would be invalid: {e:#}");
    }

    std::fs::write(config_path, &new_yaml)
        .with_context(|| format!("cannot write config file: {}", config_path.display()))?;

    println!("{key} = {value}");
    Ok(())
}
