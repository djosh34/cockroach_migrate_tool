#[path = "support/readme_contract.rs"]
mod readme_contract_support;
#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;

use readme_contract_support::RepositoryReadme;
use runner_docker_contract_support::RunnerDockerContract;
use assert_cmd::Command;
use predicates::prelude::predicate;
use std::{fs, path::PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn readme_includes_a_write_freeze_cutover_runbook_with_repeated_verify_during_shadowing() {
    let readme = RepositoryReadme::load();

    readme.assert_contains(
        "## Write-Freeze Cutover Runbook",
        "README must contain a dedicated write-freeze cutover runbook section"
    );
    readme.assert_contains(
        "run `runner verify --config <path> --mapping <id> --source-url <cockroach-url>` repeatedly while PostgreSQL is shadowing CockroachDB",
        "README must tell operators to run repeated verify checks during the shadowing period"
    );
}

#[test]
fn readme_lists_the_public_cutover_steps_in_operator_order() {
    let readme = RepositoryReadme::load();

    readme.assert_in_order(
        &[
            "1. Block writes at the API boundary for the mapping you are handing over.",
            "2. Run `runner cutover-readiness --config <path> --mapping <id> --source-url <cockroach-url>` until it reports `ready=true`.",
            "3. Run one final `runner verify --config <path> --mapping <id> --source-url <cockroach-url>` after readiness reports drained.",
            "4. Switch application traffic to PostgreSQL only after those checks finish cleanly.",
        ],
        "README must document cutover in freeze -> readiness -> final verify -> switch order"
    );
}

#[test]
fn readme_forbids_switching_until_freeze_drain_and_final_verify_all_pass() {
    let readme = RepositoryReadme::load();

    readme.assert_contains(
        "Do not switch traffic until writes are frozen, `runner cutover-readiness` has drained to zero with `ready=true`, and the final `runner verify` reports equality.",
        "README must make the cutover gate explicit"
    );
}

#[test]
fn docker_quick_start_tells_a_novice_how_to_export_the_schema_artifacts_it_requires() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    docker_quick_start.assert_contains(
        "mkdir -p schema",
        "README Docker quick start must create the schema artifact directory before compare-schema relies on it"
    );
    docker_quick_start.assert_contains(
        "cockroach sql",
        "README Docker quick start must tell a novice how to export the CockroachDB schema artifact"
    );
    docker_quick_start.assert_contains(
        "SHOW CREATE ALL TABLES;",
        "README Docker quick start must show the CockroachDB export query used to produce `/schema/crdb_schema.txt`"
    );
    docker_quick_start.assert_contains(
        "pg_dump",
        "README Docker quick start must tell a novice to use pg_dump for the PostgreSQL schema artifact"
    );
    assert!(
        docker_quick_start.contains("--schema-only")
            && docker_quick_start.contains("--no-owner")
            && docker_quick_start.contains("--no-privileges"),
        "README Docker quick start must show the schema-only pg_dump flags used to produce `/schema/pg_schema.sql`"
    );
}

#[test]
fn source_bootstrap_quick_start_shows_how_to_run_the_rendered_script() {
    let readme = RepositoryReadme::load();
    let source_bootstrap_quick_start = readme.source_bootstrap_quick_start();

    source_bootstrap_quick_start.assert_contains(
        "bash cockroach-bootstrap.sh",
        "README source bootstrap quick start must show the explicit command that runs the rendered bootstrap script"
    );
}

#[test]
fn docker_quick_start_tells_the_operator_to_run_generated_grants_before_startup() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    docker_quick_start.assert_contains(
        "run each `grants.sql` before starting the runtime",
        "README Docker quick start must tell the operator to apply the generated grants before `runner run`"
    );
}

#[test]
fn quick_start_explicitly_says_it_does_not_require_repo_internal_reading() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    docker_quick_start.assert_contains(
        "You should not need to inspect `crates/`, `tests/`, or `investigations/` to complete this quick start.",
        "README quick start must explicitly forbid repo-internal reading as part of the operator path"
    );
}

#[test]
fn docker_quick_start_documents_the_direct_runner_image_build_and_run_contract() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    RunnerDockerContract::assert_readme_documents_direct_build_and_run(docker_quick_start.text());
}

#[test]
fn docker_quick_start_forbids_wrapper_script_handoff_in_the_public_container_path() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    RunnerDockerContract::assert_readme_has_no_wrapper_handoff(docker_quick_start.text());
}

#[test]
fn docker_quick_start_runner_config_is_copyable_and_starts_with_one_mapping() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    let fixture_text = fs::read_to_string(fixture_path("readme-runner-config.yml"))
        .expect("README runner config fixture should be readable");
    assert_eq!(
        docker_quick_start.code_block("yaml"),
        fixture_text.trim_end(),
        "README runner YAML should match its canonical fixture"
    );
    fs::write(&config_path, fixture_text)
        .expect("README runner config should be writable");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"))
        .stdout(predicate::str::contains("mappings=1"));
}

#[test]
fn docker_quick_start_explicitly_creates_tls_material_before_validate_config() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    docker_quick_start.assert_contains(
        "mkdir -p config/certs",
        "README Docker quick start must create the config/certs directory before validate-config relies on it"
    );
    docker_quick_start.assert_contains(
        "openssl req",
        "README Docker quick start must include a copyable TLS-material generation command"
    );
    docker_quick_start.assert_in_order(
        &[
            "mkdir -p config/certs",
            "openssl req",
            "validate-config --config /config/runner.yml",
        ],
        "README Docker quick start must create TLS material before validate-config uses the mounted config"
    );
}

#[test]
fn docker_quick_start_explicitly_reuses_the_same_mapping_and_schema_artifacts() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    docker_quick_start.assert_contains(
        "Keep using the same `/config/runner.yml`, `app-a`, `/schema/crdb_schema.txt`, and `/schema/pg_schema.sql` values in the remaining quick-start commands unless you intentionally switch to a different mapping.",
        "README Docker quick start must make mapping-id and schema-artifact reuse explicit for novices"
    );
    docker_quick_start.assert_in_order(
        &[
            "Keep using the same `/config/runner.yml`, `app-a`, `/schema/crdb_schema.txt`, and `/schema/pg_schema.sql` values in the remaining quick-start commands unless you intentionally switch to a different mapping.",
            "compare-schema",
            "render-helper-plan",
        ],
        "README Docker quick start must explain value reuse before the later mapping-scoped commands"
    );
}
