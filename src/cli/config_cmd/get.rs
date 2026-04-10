use std::path::Path;

use anyhow::{Context, Result};
use serde_yaml::Value;

use super::dotted_path;

pub fn run(config_path: &Path, key: &str) -> Result<()> {
    let raw = std::fs::read_to_string(config_path)
        .with_context(|| format!("cannot read config file: {}", config_path.display()))?;

    let yaml: Value = serde_yaml::from_str(&raw).context("config file is not valid YAML")?;

    let segments = dotted_path::parse(key)?;
    let value = dotted_path::resolve(&yaml, &segments)?;

    match value {
        Value::String(s) => println!("{s}"),
        Value::Number(n) => println!("{n}"),
        Value::Bool(b) => println!("{b}"),
        Value::Null => println!("null"),
        // For complex values (mappings, sequences), print as YAML fragment
        other => {
            let out = serde_yaml::to_string(other).context("failed to serialize value")?;
            print!("{out}");
        }
    }

    Ok(())
}
