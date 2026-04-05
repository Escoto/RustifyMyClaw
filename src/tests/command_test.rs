use super::*;

#[test]
fn parses_new_command() {
    assert_eq!(BridgeCommand::parse("/new"), BridgeCommand::NewSession);
}

#[test]
fn parses_status_command() {
    assert_eq!(BridgeCommand::parse("/status"), BridgeCommand::Status);
}

#[test]
fn parses_help_command() {
    assert_eq!(BridgeCommand::parse("/help"), BridgeCommand::Help);
}

#[test]
fn plain_text_is_prompt() {
    assert_eq!(
        BridgeCommand::parse("hello world"),
        BridgeCommand::Prompt {
            text: "hello world".to_string()
        }
    );
}

#[test]
fn leading_trailing_whitespace_stripped_for_commands() {
    assert_eq!(BridgeCommand::parse("  /new  "), BridgeCommand::NewSession);
    assert_eq!(BridgeCommand::parse("  /help  "), BridgeCommand::Help);
}

#[test]
fn leading_whitespace_stripped_for_prompts() {
    assert_eq!(
        BridgeCommand::parse("  hello  "),
        BridgeCommand::Prompt {
            text: "hello".to_string()
        }
    );
}

#[test]
fn empty_string_is_empty_prompt() {
    assert_eq!(
        BridgeCommand::parse(""),
        BridgeCommand::Prompt {
            text: "".to_string()
        }
    );
}

#[test]
fn whitespace_only_is_empty_prompt() {
    assert_eq!(
        BridgeCommand::parse("   "),
        BridgeCommand::Prompt {
            text: "".to_string()
        }
    );
}

#[test]
fn partial_command_is_prompt() {
    assert_eq!(
        BridgeCommand::parse("/newish"),
        BridgeCommand::Prompt {
            text: "/newish".to_string()
        }
    );
}

#[test]
fn unicode_prompt() {
    let text = "Olá 🌍";
    assert_eq!(
        BridgeCommand::parse(text),
        BridgeCommand::Prompt {
            text: text.to_string()
        }
    );
}

#[test]
fn use_command_parses_workspace_name() {
    assert_eq!(
        BridgeCommand::parse("/use my-project"),
        BridgeCommand::UseWorkspace {
            name: "my-project".to_string()
        }
    );
}

#[test]
fn use_command_trims_workspace_name() {
    assert_eq!(
        BridgeCommand::parse("/use  my-project  "),
        BridgeCommand::UseWorkspace {
            name: "my-project".to_string()
        }
    );
}

#[test]
fn use_command_with_empty_name_becomes_prompt() {
    // "/use" with no argument doesn't match the "/use " prefix — falls through to Prompt.
    assert_eq!(
        BridgeCommand::parse("/use"),
        BridgeCommand::Prompt {
            text: "/use".to_string()
        }
    );
}

#[test]
fn use_command_workspace_name_with_spaces() {
    assert_eq!(
        BridgeCommand::parse("/use data pipeline"),
        BridgeCommand::UseWorkspace {
            name: "data pipeline".to_string()
        }
    );
}

#[test]
fn partial_use_is_prompt() {
    assert_eq!(
        BridgeCommand::parse("/userspace"),
        BridgeCommand::Prompt {
            text: "/userspace".to_string()
        }
    );
}
