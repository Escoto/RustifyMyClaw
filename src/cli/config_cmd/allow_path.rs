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

/// Collect parent directories that need execute traversal ACLs.
/// Returns ancestors from shallowest to deepest, excluding `/` and the path itself.
#[cfg(any(not(target_os = "windows"), test))]
fn traversal_parents(path: &Path) -> Vec<&Path> {
    let mut parents = Vec::new();
    let mut current = path.parent();
    while let Some(p) = current {
        if p == Path::new("/") {
            break;
        }
        parents.push(p);
        current = p.parent();
    }
    parents.reverse();
    parents
}

pub fn run(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let _ = path;
        bail!("allow-path is only supported on Linux with systemd");
    }

    #[cfg(not(target_os = "windows"))]
    {
        use anyhow::{bail, Context};

        const OVERRIDE_DIR: &str = "/etc/systemd/system/rustifymyclaw.service.d";
        const OVERRIDE_FILE: &str = "override.conf";
        const SERVICE_USER: &str = "rustifymyclaw";

        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("path does not exist: {}", path.display()))?;
        let canonical_str = canonical.to_string_lossy();

        // ── Fail fast: check setfacl availability ──────────────────────
        let setfacl_check = std::process::Command::new("which")
            .arg("setfacl")
            .output()
            .context("failed to check for setfacl")?;

        if !setfacl_check.status.success() {
            bail!(
                "setfacl is not installed. Install the acl package first:\n  \
                 sudo apt install acl     # Debian/Ubuntu\n  \
                 sudo dnf install acl     # Fedora/RHEL"
            );
        }

        // ── Parent directory traversal ACLs (execute only) ─────────────
        let parents = traversal_parents(&canonical);
        for parent in &parents {
            let status = std::process::Command::new("setfacl")
                .args([
                    "-m",
                    &format!("u:{SERVICE_USER}:x"),
                    &parent.to_string_lossy(),
                ])
                .status()
                .with_context(|| format!("failed to run setfacl on {}", parent.display()))?;

            if !status.success() {
                bail!(
                    "setfacl failed on {}. Are you running with sudo?",
                    parent.display()
                );
            }
        }

        if !parents.is_empty() {
            println!(
                "Set traverse (x) ACL on {} parent director{}.",
                parents.len(),
                if parents.len() == 1 { "y" } else { "ies" }
            );
        }

        // ── Workspace ACLs (recursive read/write/traverse) ────────────
        let status = std::process::Command::new("setfacl")
            .args([
                "-R",
                "-m",
                &format!("u:{SERVICE_USER}:rwX"),
                &*canonical_str,
            ])
            .status()
            .with_context(|| format!("failed to set ACLs on {}", canonical.display()))?;

        if !status.success() {
            bail!(
                "setfacl -R failed on {}. Are you running with sudo?",
                canonical.display()
            );
        }

        // ── Default ACLs (so new files inherit permissions) ────────────
        let status = std::process::Command::new("setfacl")
            .args([
                "-R",
                "-d",
                "-m",
                &format!("u:{SERVICE_USER}:rwX"),
                &*canonical_str,
            ])
            .status()
            .with_context(|| format!("failed to set default ACLs on {}", canonical.display()))?;

        if !status.success() {
            bail!(
                "setfacl -R -d failed on {}. Are you running with sudo?",
                canonical.display()
            );
        }

        println!(
            "Set read/write ACLs on {} (with default ACLs for new files).",
            canonical.display()
        );

        // ── Systemd override (ReadWritePaths) ──────────────────────────
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
                println!(
                    "{} is already in the systemd allowed paths.",
                    canonical.display()
                );
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

                println!(
                    "Added ReadWritePaths={} to systemd override.",
                    canonical.display()
                );
            }
        }

        println!();
        println!("Reload and restart the service to apply:");
        println!("  sudo systemctl daemon-reload && sudo systemctl restart rustifymyclaw");

        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/cli/config_cmd/allow_path_test.rs"]
mod tests;
