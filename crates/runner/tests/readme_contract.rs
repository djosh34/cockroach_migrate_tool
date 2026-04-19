#[path = "support/readme_contract.rs"]
mod readme_contract_support;

use readme_contract_support::RepositoryReadme;

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
