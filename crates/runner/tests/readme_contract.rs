#[path = "support/readme_contract.rs"]
mod readme_contract_support;
#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;

use assert_cmd::Command;
use predicates::prelude::predicate;
use readme_contract_support::RepositoryReadme;
use runner_docker_contract_support::RunnerDockerContract;
use std::{fs, path::PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn source_bootstrap_quick_start_shows_the_sql_only_contract() {
    let readme = RepositoryReadme::load();
    let source_bootstrap_quick_start = readme.source_bootstrap_quick_start();

    source_bootstrap_quick_start.assert_contains(
        "render-bootstrap-sql",
        "README source bootstrap quick start must show the SQL-only source setup command",
    );
    source_bootstrap_quick_start.assert_contains(
        "cockroach-bootstrap.sql",
        "README source bootstrap quick start must render a SQL artifact instead of a shell script",
    );
    assert!(
        !source_bootstrap_quick_start.contains("bash cockroach-bootstrap.sh"),
        "README source bootstrap quick start must not tell operators to execute a rendered shell script"
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
    fs::write(&config_path, fixture_text).expect("README runner config should be writable");

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
        "README Docker quick start must include a copyable TLS-material generation command",
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
fn docker_quick_start_keeps_runner_destination_only() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    docker_quick_start.assert_in_order(
        &[
            "validate-config --config /config/runner.yml",
            "render-postgres-setup --config /config/runner.yml --output-dir /work/postgres-setup",
            "run --config /config/runner.yml",
        ],
        "README Docker quick start must present only the destination-side runner commands in operator order"
    );
    for forbidden_marker in [
        "compare-schema",
        "render-helper-plan",
        "verify",
        "cutover-readiness",
        "--source-url",
        "--cockroach-schema",
    ] {
        assert!(
            !docker_quick_start.contains(forbidden_marker),
            "README Docker quick start must not expose removed runner surface `{forbidden_marker}`",
        );
    }
}
