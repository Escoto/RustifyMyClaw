use std::path::Path;

use anyhow::Result;

pub fn run(config_path: &Path) -> Result<()> {
    println!("{}", config_path.display());
    Ok(())
}
