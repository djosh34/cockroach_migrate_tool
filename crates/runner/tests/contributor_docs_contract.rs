use std::{fs, path::PathBuf};

fn contributing_doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("CONTRIBUTING.md")
}

#[test]
fn contributing_doc_preserves_workspace_and_validation_guidance() {
    let contributing =
        fs::read_to_string(contributing_doc_path()).expect("CONTRIBUTING.md should be readable");

    assert!(
        contributing.contains("## Workspace Layout"),
        "CONTRIBUTING.md must preserve the workspace layout guidance moved out of README"
    );
    assert!(
        contributing.contains("## Command Contract"),
        "CONTRIBUTING.md must preserve the contributor command contract moved out of README"
    );
    assert!(
        contributing.contains("`make check`"),
        "CONTRIBUTING.md must document the contributor validation commands"
    );
    assert!(
        contributing.contains("`cargo test --workspace`"),
        "CONTRIBUTING.md must preserve the narrower Cargo test loop"
    );
}
