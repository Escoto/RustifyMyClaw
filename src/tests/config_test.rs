use super::*;

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
