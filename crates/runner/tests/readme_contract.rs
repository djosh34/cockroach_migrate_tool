use std::{fs, path::PathBuf};

fn repository_readme_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("README.md")
}

fn repository_readme() -> String {
    fs::read_to_string(repository_readme_path()).expect("repository README should be readable")
}

fn phrase_offset(text: &str, phrase: &str) -> usize {
    text.find(phrase)
        .unwrap_or_else(|| panic!("README must contain `{phrase}`"))
}

#[test]
fn readme_includes_a_write_freeze_cutover_runbook_with_repeated_verify_during_shadowing() {
    let readme = repository_readme();

    assert!(
        readme.contains("## Write-Freeze Cutover Runbook"),
        "README must contain a dedicated write-freeze cutover runbook section"
    );
    assert!(
        readme.contains("run `runner verify --config <path> --mapping <id> --source-url <cockroach-url>` repeatedly while PostgreSQL is shadowing CockroachDB"),
        "README must tell operators to run repeated verify checks during the shadowing period"
    );
}

#[test]
fn readme_lists_the_public_cutover_steps_in_operator_order() {
    let readme = repository_readme();

    let freeze = phrase_offset(
        &readme,
        "1. Block writes at the API boundary for the mapping you are handing over.",
    );
    let readiness = phrase_offset(
        &readme,
        "2. Run `runner cutover-readiness --config <path> --mapping <id> --source-url <cockroach-url>` until it reports `ready=true`.",
    );
    let final_verify = phrase_offset(
        &readme,
        "3. Run one final `runner verify --config <path> --mapping <id> --source-url <cockroach-url>` after readiness reports drained.",
    );
    let switch = phrase_offset(
        &readme,
        "4. Switch application traffic to PostgreSQL only after those checks finish cleanly.",
    );

    assert!(
        freeze < readiness && readiness < final_verify && final_verify < switch,
        "README must document cutover in freeze -> readiness -> final verify -> switch order"
    );
}

#[test]
fn readme_forbids_switching_until_freeze_drain_and_final_verify_all_pass() {
    let readme = repository_readme();

    assert!(
        readme.contains(
            "Do not switch traffic until writes are frozen, `runner cutover-readiness` has drained to zero with `ready=true`, and the final `runner verify` reports equality."
        ),
        "README must make the cutover gate explicit"
    );
}
