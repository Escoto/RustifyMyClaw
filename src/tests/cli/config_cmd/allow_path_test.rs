use super::*;

#[test]
fn empty_existing_creates_service_section() {
    let result = merge_allowed_path("", "/home/user/project-a");
    match result {
        MergeResult::Updated(content) => {
            assert_eq!(content, "[Service]\nReadWritePaths=/home/user/project-a\n");
        }
        MergeResult::AlreadyPresent => panic!("expected Updated, got AlreadyPresent"),
    }
}

#[test]
fn second_path_appends_under_service_section() {
    let existing = "[Service]\nReadWritePaths=/home/user/project-a\n";
    let result = merge_allowed_path(existing, "/home/user/project-b");
    match result {
        MergeResult::Updated(content) => {
            assert!(
                content.contains("ReadWritePaths=/home/user/project-a"),
                "first path should be preserved, got:\n{content}"
            );
            assert!(
                content.contains("ReadWritePaths=/home/user/project-b"),
                "second path should be added, got:\n{content}"
            );
        }
        MergeResult::AlreadyPresent => panic!("expected Updated, got AlreadyPresent"),
    }
}

#[test]
fn three_paths_all_preserved() {
    // Add first.
    let mut content = match merge_allowed_path("", "/path/a") {
        MergeResult::Updated(c) => c,
        MergeResult::AlreadyPresent => panic!("first path should be new"),
    };
    // Add second.
    match merge_allowed_path(&content, "/path/b") {
        MergeResult::Updated(c) => content = c,
        MergeResult::AlreadyPresent => panic!("second path should be new"),
    }
    // Add third.
    match merge_allowed_path(&content, "/path/c") {
        MergeResult::Updated(c) => content = c,
        MergeResult::AlreadyPresent => panic!("third path should be new"),
    }

    assert!(
        content.contains("ReadWritePaths=/path/a"),
        "missing /path/a"
    );
    assert!(
        content.contains("ReadWritePaths=/path/b"),
        "missing /path/b"
    );
    assert!(
        content.contains("ReadWritePaths=/path/c"),
        "missing /path/c"
    );

    // Exactly one [Service] header.
    let service_count = content.matches("[Service]").count();
    assert_eq!(
        service_count, 1,
        "expected one [Service] section, got {service_count}"
    );
}

#[test]
fn duplicate_path_is_detected() {
    let existing = "[Service]\nReadWritePaths=/home/user/project-a\n";
    let result = merge_allowed_path(existing, "/home/user/project-a");
    assert!(
        matches!(result, MergeResult::AlreadyPresent),
        "duplicate path should return AlreadyPresent"
    );
}
