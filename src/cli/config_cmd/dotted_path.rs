use anyhow::{bail, Result};
use serde_yaml::Value;

/// A single segment in a dotted path: either a map key or a sequence index.
#[derive(Debug, PartialEq)]
pub enum Segment {
    Key(String),
    Index(usize),
}

/// Parse a dotted-path expression like `workspaces[0].channels[1].kind` into segments.
///
/// Grammar:
///   path     = segment ('.' segment)*
///   segment  = identifier ('[' number ']')*
pub fn parse(input: &str) -> Result<Vec<Segment>> {
    if input.is_empty() {
        bail!("dotted path cannot be empty");
    }

    let mut segments = Vec::new();
    let mut chars = input.chars().peekable();
    let mut buf = String::new();

    while chars.peek().is_some() {
        buf.clear();

        // Read identifier (key name)
        while let Some(&c) = chars.peek() {
            if c == '.' || c == '[' {
                break;
            }
            buf.push(c);
            chars.next();
        }

        if !buf.is_empty() {
            segments.push(Segment::Key(buf.clone()));
        }

        // Read any bracket indices
        while chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            let mut idx_buf = String::new();
            while let Some(&c) = chars.peek() {
                if c == ']' {
                    break;
                }
                idx_buf.push(c);
                chars.next();
            }
            if chars.next() != Some(']') {
                bail!("unclosed bracket in path: {input}");
            }
            let idx: usize = idx_buf
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid index `{idx_buf}` in path: {input}"))?;
            segments.push(Segment::Index(idx));
        }

        // Consume dot separator
        if chars.peek() == Some(&'.') {
            chars.next();
        }
    }

    if segments.is_empty() {
        bail!("dotted path produced no segments: {input}");
    }

    Ok(segments)
}

/// Navigate a `serde_yaml::Value` tree by segments and return a reference to the leaf.
pub fn resolve<'a>(root: &'a Value, segments: &[Segment]) -> Result<&'a Value> {
    let mut current = root;
    for seg in segments {
        match seg {
            Segment::Key(key) => {
                let map = current
                    .as_mapping()
                    .ok_or_else(|| anyhow::anyhow!("expected mapping at key `{key}`"))?;
                current = map
                    .get(Value::String(key.clone()))
                    .ok_or_else(|| anyhow::anyhow!("key `{key}` not found"))?;
            }
            Segment::Index(idx) => {
                let seq = current
                    .as_sequence()
                    .ok_or_else(|| anyhow::anyhow!("expected sequence at index [{idx}]"))?;
                current = seq.get(*idx).ok_or_else(|| {
                    anyhow::anyhow!("index [{idx}] out of range (len {})", seq.len())
                })?;
            }
        }
    }
    Ok(current)
}

/// Navigate a `serde_yaml::Value` tree by segments and return a mutable reference to the leaf.
pub fn resolve_mut<'a>(root: &'a mut Value, segments: &[Segment]) -> Result<&'a mut Value> {
    let mut current = root;
    for seg in segments {
        match seg {
            Segment::Key(key) => {
                let map = current
                    .as_mapping_mut()
                    .ok_or_else(|| anyhow::anyhow!("expected mapping at key `{key}`"))?;
                current = map.entry(Value::String(key.clone())).or_insert(Value::Null);
            }
            Segment::Index(idx) => {
                let seq = current
                    .as_sequence_mut()
                    .ok_or_else(|| anyhow::anyhow!("expected sequence at index [{idx}]"))?;
                let len = seq.len();
                current = seq
                    .get_mut(*idx)
                    .ok_or_else(|| anyhow::anyhow!("index [{idx}] out of range (len {len})"))?;
            }
        }
    }
    Ok(current)
}

/// Auto-detect a string value and convert to the appropriate YAML type.
pub fn auto_value(raw: &str) -> Value {
    if let Ok(n) = raw.parse::<i64>() {
        return Value::Number(n.into());
    }
    if let Ok(n) = raw.parse::<f64>() {
        return Value::Number(serde_yaml::Number::from(n));
    }
    match raw {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" | "~" => Value::Null,
        _ => Value::String(raw.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
