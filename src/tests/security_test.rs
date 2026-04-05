use super::*;

fn gate(ids: &[&str]) -> SecurityGate {
    SecurityGate::new(ids.iter().map(|s| s.to_string()).collect())
}

#[test]
fn allowed_user_passes() {
    let g = gate(&["123456", "@user-x"]);
    assert!(g.is_allowed("123456"));
    assert!(g.is_allowed("@user-x"));
}

#[test]
fn blocked_user_rejected() {
    let g = gate(&["123456"]);
    assert!(!g.is_allowed("999999"));
}

#[test]
fn empty_allowlist_blocks_all() {
    let g = gate(&[]);
    assert!(!g.is_allowed("anyone"));
    assert!(!g.is_allowed(""));
}

#[test]
fn exact_match_required() {
    let g = gate(&["@user-x"]);
    assert!(!g.is_allowed("user-x"));
}
