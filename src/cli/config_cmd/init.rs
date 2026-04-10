use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::{bail, Result};

/// Prompt user for Y/n confirmation. Returns `true` on yes (default).
fn confirm_overwrite(path: &Path) -> Result<bool> {
    print!("file already exists: {}\nOverwrite? [Y/n] ", path.display());
    io::stdout().flush()?;

    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let answer = line.trim().to_lowercase();

    Ok(answer.is_empty() || answer == "y" || answer == "yes")
}

const TEMPLATE: &str = r#"# RustifyMyClaw configuration
# Full field reference: docs/configuration.md

workspaces:
  - name: "my-project"
    directory: "/path/to/your/project"       # must exist on disk
    backend: "claude-cli"                     # claude-cli | codex-cli | gemini-cli
    # timeout_seconds: 300                    # optional: CLI process timeout
    channels:
      - kind: telegram                        # telegram | whatsapp | slack
        token: "${TELEGRAM_BOT_TOKEN}"
        # bot_name: "MyBot"                   # optional: display name
        allowed_users:
          - 123456789                         # numeric user ID
          # - "@username"                     # or handle string

        ## Per-channel output overrides (optional):
        # max_message_chars: 4000
        # file_upload_threshold_bytes: 51200

        ## WhatsApp-specific (ignored on other channels):
        # phone_number_id: "${WA_PHONE_NUMBER_ID}"
        # webhook_port: 8080
        # verify_token: "${WA_VERIFY_TOKEN}"

        ## Slack-specific (ignored on other channels):
        # app_token: "${SLACK_APP_TOKEN}"
        # use_threads: false

output:
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200          # 50 KB
  chunk_strategy: "natural"                   # natural | fixed

# Optional: per-user rate limiting
# limits:
#   max_requests: 10
#   window_seconds: 60
"#;

pub fn run(file: Option<&Path>, dir: Option<&Path>, default_config_path: &Path) -> Result<()> {
    let target = if let Some(d) = dir {
        d.join("config.yaml")
    } else if let Some(f) = file {
        f.to_path_buf()
    } else {
        default_config_path.to_path_buf()
    };
    let target = target.as_path();

    if target.exists() && !confirm_overwrite(target)? {
        bail!("aborted");
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(target, TEMPLATE)?;
    println!("config template written to: {}", target.display());
    Ok(())
}
