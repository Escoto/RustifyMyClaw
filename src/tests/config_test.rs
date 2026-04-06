use super::*;

fn default_output() -> OutputConfig {
    OutputConfig {
        max_message_chars: 4000,
        file_upload_threshold_bytes: 51200,
        chunk_strategy: ChunkStrategy::Natural,
    }
}

fn make_config(yaml: &str) -> Result<AppConfig> {
    let config: AppConfig = serde_yaml::from_str(yaml).context("malformed YAML")?;
    validate(&config)?;
    Ok(config)
}

const VALID_YAML: &str = r#"
workspaces:
  - name: "test-ws"
    directory: "/tmp"
    backend: "claude-cli"
    channels:
      - kind: telegram
        token: "tok"
        allowed_users:
          - "@user-x"
output:
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"
"#;

#[test]
fn valid_config_parses() {
    let cfg = make_config(VALID_YAML).unwrap();
    assert_eq!(cfg.workspaces[0].name, "test-ws");
    assert_eq!(cfg.output.chunk_strategy, ChunkStrategy::Natural);
}

#[test]
fn unknown_backend_is_rejected() {
    let yaml = VALID_YAML.replace("claude-cli", "gpt-cli");
    assert!(make_config(&yaml).is_err());
}

#[test]
fn empty_allowed_users_is_rejected() {
    let yaml = VALID_YAML.replace("          - \"@user-x\"", "");
    assert!(make_config(&yaml).is_err());
}

#[test]
fn unknown_channel_kind_is_rejected() {
    let yaml = VALID_YAML.replace("telegram", "discord");
    assert!(make_config(&yaml).is_err());
}

#[test]
fn missing_output_section_is_rejected() {
    let yaml = r#"
workspaces:
  - name: "test-ws"
    directory: "/tmp"
    backend: "claude-cli"
    channels:
      - kind: telegram
        token: "tok"
        allowed_users:
          - "@user-x"
"#;
    assert!(serde_yaml::from_str::<AppConfig>(yaml).is_err());
}

#[test]
fn numeric_and_handle_allowed_users_parse() {
    let yaml = r#"
workspaces:
  - name: "test-ws"
    directory: "/tmp"
    backend: "claude-cli"
    channels:
      - kind: telegram
        token: "tok"
        allowed_users:
          - "@user-x"
          - 987654321
output:
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"
"#;
    let cfg = make_config(yaml).unwrap();
    assert_eq!(cfg.workspaces[0].channels[0].allowed_users.len(), 2);
}

#[test]
fn env_var_interpolation_replaces_known_vars() {
    std::env::set_var("TEST_TOKEN_BRIDGECLI", "secret123");
    let raw = "token: ${TEST_TOKEN_BRIDGECLI}";
    let result = interpolate_env_vars(raw).unwrap();
    assert_eq!(result, "token: secret123");
    std::env::remove_var("TEST_TOKEN_BRIDGECLI");
}

#[test]
fn env_var_interpolation_fails_on_missing_var() {
    std::env::remove_var("BRIDGECLI_DEFINITELY_NOT_SET");
    let raw = "token: ${BRIDGECLI_DEFINITELY_NOT_SET}";
    assert!(interpolate_env_vars(raw).is_err());
}

#[test]
fn effective_output_uses_channel_override() {
    let global = default_output();
    let channel = ChannelConfig {
        kind: "slack".to_string(),
        bot_name: None,
        token: "tok".to_string(),
        allowed_users: vec![],
        max_message_chars: Some(3000),
        file_upload_threshold_bytes: None,
        webhook_port: None,
        phone_number_id: None,
        verify_token: None,
        app_token: None,
        use_threads: None,
    };
    let effective = effective_output_config(&global, &channel);
    assert_eq!(effective.max_message_chars, 3000);
    assert_eq!(effective.file_upload_threshold_bytes, 51200);
}

#[test]
fn effective_output_falls_back_to_global() {
    let global = default_output();
    let channel = ChannelConfig {
        kind: "telegram".to_string(),
        bot_name: None,
        token: "tok".to_string(),
        allowed_users: vec![],
        max_message_chars: None,
        file_upload_threshold_bytes: None,
        webhook_port: None,
        phone_number_id: None,
        verify_token: None,
        app_token: None,
        use_threads: None,
    };
    let effective = effective_output_config(&global, &channel);
    assert_eq!(effective.max_message_chars, 4000);
    assert_eq!(effective.file_upload_threshold_bytes, 51200);
}

#[test]
fn whatsapp_config_fields_parse() {
    let yaml = r#"
workspaces:
  - name: "test-ws"
    directory: "/tmp"
    backend: "claude-cli"
    channels:
      - kind: whatsapp
        token: "wa-token"
        phone_number_id: "12345"
        webhook_port: 8080
        verify_token: "secret"
        max_message_chars: 2000
        allowed_users:
          - "+5511999999999"
output:
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"
"#;
    let cfg: AppConfig = serde_yaml::from_str(yaml).unwrap();
    let ch = &cfg.workspaces[0].channels[0];
    assert_eq!(ch.phone_number_id.as_deref(), Some("12345"));
    assert_eq!(ch.webhook_port, Some(8080));
    assert_eq!(ch.verify_token.as_deref(), Some("secret"));
    assert_eq!(ch.max_message_chars, Some(2000));
}

#[test]
fn slack_config_fields_parse() {
    let yaml = r#"
workspaces:
  - name: "test-ws"
    directory: "/tmp"
    backend: "claude-cli"
    channels:
      - kind: slack
        token: "xoxb-bot-token"
        app_token: "xapp-app-token"
        use_threads: true
        max_message_chars: 3000
        allowed_users:
          - "@dev_user"
output:
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"
"#;
    let cfg: AppConfig = serde_yaml::from_str(yaml).unwrap();
    let ch = &cfg.workspaces[0].channels[0];
    assert_eq!(ch.app_token.as_deref(), Some("xapp-app-token"));
    assert_eq!(ch.use_threads, Some(true));
    assert_eq!(ch.max_message_chars, Some(3000));
}

#[test]
fn telegram_config_still_parses_without_new_fields() {
    let cfg = make_config(VALID_YAML).unwrap();
    let ch = &cfg.workspaces[0].channels[0];
    assert!(ch.max_message_chars.is_none());
    assert!(ch.app_token.is_none());
    assert!(ch.webhook_port.is_none());
}

#[test]
fn whatsapp_fields_on_telegram_channel_still_parse() {
    // Misplaced fields are silently deserialized (warn_misplaced_fields emits tracing::warn,
    // but validation does not error — the daemon starts and ignores the rogue fields).
    let yaml = r#"
workspaces:
  - name: "test-ws"
    directory: "/tmp"
    backend: "claude-cli"
    channels:
      - kind: telegram
        token: "tok"
        phone_number_id: "oops"
        app_token: "also-oops"
        allowed_users:
          - "@user-x"
output:
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"
"#;
    // Validation should succeed — misplaced fields warn but do not bail.
    let cfg = make_config(yaml).unwrap();
    let ch = &cfg.workspaces[0].channels[0];
    assert_eq!(ch.phone_number_id.as_deref(), Some("oops"));
    assert_eq!(ch.app_token.as_deref(), Some("also-oops"));
}

#[test]
fn empty_workspaces_is_rejected() {
    let yaml = r#"
workspaces: []
output:
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"
"#;
    assert!(make_config(yaml).is_err());
}
