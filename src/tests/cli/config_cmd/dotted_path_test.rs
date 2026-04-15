use super::*;
use serde_yaml::Value;

#[test]
fn parse_simple_key() {
    let segs = parse("output").unwrap();
    assert_eq!(segs, vec![Segment::Key("output".into())]);
}

#[test]
fn parse_dotted_keys() {
    let segs = parse("output.max_message_chars").unwrap();
    assert_eq!(
        segs,
        vec![
            Segment::Key("output".into()),
            Segment::Key("max_message_chars".into()),
        ]
    );
}

#[test]
fn parse_with_index() {
    let segs = parse("workspaces[0].channels[1].kind").unwrap();
    assert_eq!(
        segs,
        vec![
            Segment::Key("workspaces".into()),
            Segment::Index(0),
            Segment::Key("channels".into()),
            Segment::Index(1),
            Segment::Key("kind".into()),
        ]
    );
}

#[test]
fn parse_empty_errors() {
    assert!(parse("").is_err());
}

#[test]
fn parse_unclosed_bracket_errors() {
    assert!(parse("workspaces[0").is_err());
}

#[test]
fn auto_value_int() {
    assert_eq!(auto_value("42"), Value::Number(42.into()));
}

#[test]
fn auto_value_bool() {
    assert_eq!(auto_value("true"), Value::Bool(true));
    assert_eq!(auto_value("false"), Value::Bool(false));
}

#[test]
fn auto_value_string() {
    assert_eq!(auto_value("hello"), Value::String("hello".into()));
}

#[test]
fn auto_value_null() {
    assert_eq!(auto_value("null"), Value::Null);
}

#[test]
fn resolve_nested_value() {
    let yaml: Value = serde_yaml::from_str("a:\n  b:\n    - x\n    - y").unwrap();
    let segs = parse("a.b[1]").unwrap();
    let val = resolve(&yaml, &segs).unwrap();
    assert_eq!(val, &Value::String("y".into()));
}

#[test]
fn resolve_missing_key_errors() {
    let yaml: Value = serde_yaml::from_str("a: 1").unwrap();
    let segs = parse("b").unwrap();
    assert!(resolve(&yaml, &segs).is_err());
}

#[test]
fn resolve_mut_creates_missing_key() {
    let mut yaml: Value = serde_yaml::from_str("a: {}").unwrap();
    let segs = parse("a.new_key").unwrap();
    let val = resolve_mut(&mut yaml, &segs).unwrap();
    assert_eq!(val, &Value::Null);
}
