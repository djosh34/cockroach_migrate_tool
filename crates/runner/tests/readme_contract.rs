#[path = "support/published_image_contract.rs"]
mod published_image_contract_support;
#[path = "support/readme_contract.rs"]
mod readme_contract_support;
#[path = "support/readme_published_image_contract.rs"]
mod readme_published_image_contract_support;
#[path = "support/runner_public_contract.rs"]
mod runner_public_contract_support;

use assert_cmd::Command;
use predicates::prelude::predicate;
use readme_contract_support::RepositoryReadme;
use readme_published_image_contract_support::ReadmePublishedImageContract;
use runner_public_contract_support::RunnerPublicContract;
use std::{fs, path::PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn setup_sql_quick_start_shows_the_two_sql_only_commands() {
    let readme = RepositoryReadme::load();
    let setup_sql_quick_start = readme.setup_sql_quick_start();

    setup_sql_quick_start.assert_contains(
        "emit-cockroach-sql",
        "README setup-sql quick start must show the Cockroach SQL emission command",
    );
    setup_sql_quick_start.assert_contains(
        "emit-postgres-grants",
        "README setup-sql quick start must show the PostgreSQL grant emission command",
    );
    setup_sql_quick_start.assert_contains(
        "cockroach-bootstrap.sql",
        "README setup-sql quick start must render a Cockroach SQL artifact instead of a shell script",
    );
    setup_sql_quick_start.assert_contains(
        "postgres-grants.sql",
        "README setup-sql quick start must render a PostgreSQL SQL artifact instead of README trees",
    );
    assert!(
        !setup_sql_quick_start.contains("bash cockroach-bootstrap.sh"),
        "README setup-sql quick start must not tell operators to execute a rendered shell script"
    );
    ReadmePublishedImageContract::assert_setup_sql_quick_start_uses_published_image(
        setup_sql_quick_start.text(),
    );
}

#[test]
fn docker_quick_start_tells_the_operator_to_run_generated_grants_before_startup() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    docker_quick_start.assert_contains(
        "apply the emitted PostgreSQL grant SQL before starting the runtime",
        "README Docker quick start must tell the operator to apply the emitted PostgreSQL grants before `runner run`"
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
fn readme_excludes_contributor_only_workspace_and_validation_guidance() {
    let readme = RepositoryReadme::load();

    assert!(
        !readme.text().contains("## Workspace Layout"),
        "README must not contain a contributor-only workspace layout section"
    );
    assert!(
        !readme.text().contains("## Command Contract"),
        "README must not contain contributor-only command contract guidance"
    );
    assert!(
        !readme.text().contains("`make check`"),
        "README must not require operators to learn contributor validation commands"
    );
    assert!(
        !readme.text().contains("`make test`"),
        "README must not require operators to learn contributor validation commands"
    );
}

#[test]
fn readme_redirects_contributors_to_contributing_doc() {
    let readme = RepositoryReadme::load();

    assert!(
        readme
            .text()
            .contains("For contributor workflow, see `CONTRIBUTING.md`."),
        "README must redirect contributors to CONTRIBUTING.md instead of embedding contributor-only workflow rules"
    );
}

#[test]
fn docker_quick_start_documents_the_direct_runner_image_build_and_run_contract() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    ReadmePublishedImageContract::assert_runner_quick_start_uses_published_image(
        docker_quick_start.text(),
    );
}

#[test]
fn docker_quick_start_forbids_wrapper_script_handoff_in_the_public_container_path() {
    let readme = RepositoryReadme::load();
    let docker_quick_start = readme.docker_quick_start();

    ReadmePublishedImageContract::assert_readme_has_no_wrapper_handoff(docker_quick_start.text());
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
            "run --config /config/runner.yml",
        ],
        "README Docker quick start must present only the destination-side runner commands in operator order"
    );
    RunnerPublicContract::assert_text_excludes_removed_surface(
        docker_quick_start.text(),
        "README Docker quick start must not expose removed runner surface",
    );
    ReadmePublishedImageContract::assert_text_excludes_local_novice_steps(
        docker_quick_start.text(),
        "README Docker quick start must keep the novice path on published images only",
    );
}

#[test]
fn setup_sql_quick_start_forbids_repo_checkout_and_local_tooling_steps() {
    let readme = RepositoryReadme::load();
    let setup_sql_quick_start = readme.setup_sql_quick_start();

    ReadmePublishedImageContract::assert_text_excludes_local_novice_steps(
        setup_sql_quick_start.text(),
        "README setup-sql quick start must keep the novice path on published images only",
    );
}
