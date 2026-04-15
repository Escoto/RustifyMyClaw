use std::path::Path;

#[cfg(target_os = "windows")]
use anyhow::bail;
use anyhow::Result;

/// Result of merging a new path into existing override content.
#[cfg(any(not(target_os = "windows"), test))]
enum MergeResult {
    /// Path was already present; no changes needed.
    AlreadyPresent,
    /// New content to write.
    Updated(String),
}

/// Pure logic: merge a new `ReadWritePaths=` entry into existing override content.
#[cfg(any(not(target_os = "windows"), test))]
fn merge_allowed_path(existing: &str, path_str: &str) -> MergeResult {
    let entry = format!("ReadWritePaths={path_str}");

    if existing.lines().any(|line| line.trim() == entry) {
        return MergeResult::AlreadyPresent;
    }

    let new_content = if existing.is_empty() {
        format!("[Service]\n{entry}\n")
    } else if existing.contains("[Service]") {
        let mut result = String::new();
        for line in existing.lines() {
            result.push_str(line);
            result.push('\n');
            if line.trim() == "[Service]" {
                result.push_str(&entry);
                result.push('\n');
            }
        }
        result
    } else {
        format!("{existing}\n[Service]\n{entry}\n")
    };

    MergeResult::Updated(new_content)
}

pub fn run(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let _ = path;
        bail!("allow-path is only supported on Linux with systemd");
    }

    #[cfg(not(target_os = "windows"))]
    {
        use anyhow::Context;

        const OVERRIDE_DIR: &str = "/etc/systemd/system/rustifymyclaw.service.d";
        const OVERRIDE_FILE: &str = "override.conf";

        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("path does not exist: {}", path.display()))?;
        let canonical_str = canonical.to_string_lossy();

        let override_dir = Path::new(OVERRIDE_DIR);
        let override_path = override_dir.join(OVERRIDE_FILE);

        let existing = if override_path.exists() {
            std::fs::read_to_string(&override_path)
                .with_context(|| format!("cannot read {}", override_path.display()))?
        } else {
            String::new()
        };

        match merge_allowed_path(&existing, &canonical_str) {
            MergeResult::AlreadyPresent => {
                println!("{} is already in the allowed paths.", canonical.display());
            }
            MergeResult::Updated(new_content) => {
                std::fs::create_dir_all(override_dir).with_context(|| {
                    format!("cannot create {OVERRIDE_DIR}. Are you running with sudo?")
                })?;

                std::fs::write(&override_path, &new_content).with_context(|| {
                    format!(
                        "cannot write {}. Are you running with sudo?",
                        override_path.display()
                    )
                })?;

                println!("Allowed workspace path: {}", canonical.display());
                println!();
                println!("Reload and restart the service to apply:");
                println!("  sudo systemctl daemon-reload && sudo systemctl restart rustifymyclaw");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/cli/config_cmd/allow_path_test.rs"]
mod tests;
