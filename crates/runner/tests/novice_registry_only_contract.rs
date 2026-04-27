use std::{
    any::Any,
    fs,
    panic::{self, AssertUnwindSafe},
};

#[path = "support/novice_registry_only_harness.rs"]
mod novice_registry_only_harness_support;
#[path = "support/published_image_refs.rs"]
mod published_image_refs_support;
#[path = "support/readme_operator_workspace.rs"]
mod readme_operator_workspace_support;

use novice_registry_only_harness_support::NoviceRegistryOnlyHarness;

#[test]
fn setup_sql_compose_emits_sql_from_a_repo_free_operator_workspace() {
    let harness = NoviceRegistryOnlyHarness::start();

    let cockroach_sql = harness.run_setup_sql_compose_emit_cockroach_sql();

    assert!(
        cockroach_sql.starts_with("-- Source bootstrap SQL\n"),
        "setup-sql compose contract must emit SQL from a copied operator workspace",
    );
    assert!(
        cockroach_sql.contains("CREATE CHANGEFEED FOR TABLE demo_a.public.customers"),
        "setup-sql compose contract must render the README-style Cockroach mapping",
    );
}

#[test]
fn runner_readme_commands_work_from_a_repo_free_operator_workspace() {
    let harness = NoviceRegistryOnlyHarness::start();

    let validate_output = harness.run_runner_readme_validate_config();

    assert!(
        validate_output.stdout.is_empty(),
        "runner validate-config json mode must keep stdout empty",
    );
    assert!(
        validate_output
            .stderr
            .contains("\"event\":\"config.validated\""),
        "runner validate-config json mode must emit the validation event",
    );

    let runtime = harness.start_runner_readme_runtime();
    runtime.wait_for_health();
}

#[test]
fn copied_compose_contracts_work_from_a_repo_free_operator_workspace() {
    let harness = NoviceRegistryOnlyHarness::start();

    let grants_sql = harness.run_setup_sql_compose_emit_postgres_grants();
    assert!(
        grants_sql.contains(
            r#"GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE "public"."customers" TO "migration_user_a";"#,
        ),
        "setup-sql compose contract must emit PostgreSQL grants from copied operator files",
    );

    let runner_validate_output = harness.run_runner_compose_validate_config();
    assert!(
        runner_validate_output
            .stderr
            .contains("\"event\":\"config.validated\""),
        "runner compose contract must validate config through the published image only",
    );

    let verify_runtime = harness.start_verify_compose_runtime();
    verify_runtime.wait_until_running();
    verify_runtime.shutdown();
}

#[test]
fn verify_compose_runtime_serves_the_https_api_with_readme_owned_mtls_material() {
    let harness = NoviceRegistryOnlyHarness::start();
    let verify_runtime = harness.start_verify_compose_runtime();
    verify_runtime.wait_until_running();

    let status = verify_runtime.readiness_probe_status();

    assert_eq!(
        status, 404,
        "verify compose runtime must serve the HTTPS API from the README-owned mTLS workspace",
    );

    verify_runtime.shutdown();
}

#[test]
fn verify_readme_http_flow_uses_the_flat_start_contract_and_surfaces_failures() {
    let harness = NoviceRegistryOnlyHarness::start();
    let verify_runtime = harness.start_verify_compose_runtime();
    verify_runtime.wait_until_running();

    let start = verify_runtime.start_job_with_flat_filters("^public$", "^(accounts|orders)$");
    assert_eq!(
        start.status, "running",
        "verify README flow should accept the flat start contract through the published image",
    );
    assert!(
        start.job_id.starts_with("job-"),
        "verify README flow should return a concrete job id, got `{}`",
        start.job_id,
    );

    let terminal = verify_runtime.wait_for_terminal_job(&start.job_id);
    assert_eq!(
        terminal.job_id, start.job_id,
        "verify README polling flow should keep the same job id across responses",
    );
    assert_eq!(
        terminal.status, "failed",
        "verify README flow should surface the published-image failure response when the copied config points at unreachable databases",
    );
    let failure = terminal.failure.expect(
        "verify README flow should return a typed failure object for unreachable databases",
    );
    assert_eq!(failure.category, "source_access");
    assert_eq!(failure.code, "connection_failed");
    assert!(
        failure.message.contains("source connection failed"),
        "verify README flow should keep an operator-facing source failure message: {}",
        failure.message,
    );

    let validation_error = verify_runtime.start_job_with_legacy_filters_error();
    assert_eq!(
        validation_error.error.category, "request_validation",
        "verify README flow should classify the removed nested filter contract as request validation",
    );
    assert_eq!(
        validation_error.error.code, "unknown_field",
        "verify README flow should keep the documented stable validation code",
    );
    assert_eq!(
        validation_error.error.message, "request body contains an unsupported field",
        "verify README flow should reject the removed nested filter contract with the documented validation message",
    );

    verify_runtime.shutdown();
}

#[test]
fn runner_readme_runtime_distinguishes_authentication_and_connectivity_failures() {
    let harness = NoviceRegistryOnlyHarness::start();
    let postgres = harness.start_runner_destination_postgres();

    let auth_failure = harness.run_runner_readme_runtime_failure(
        "host.docker.internal",
        postgres.host_port(),
        "wrong-secret",
    );
    assert!(
        auth_failure
            .stderr
            .contains("password authentication failed"),
        "runner README runtime must surface authentication failures clearly; got stderr:\n{}",
        auth_failure.stderr,
    );
    assert!(
        !auth_failure
            .stderr
            .to_lowercase()
            .contains("connection refused"),
        "authentication failures must not be mislabeled as connectivity failures; got stderr:\n{}",
        auth_failure.stderr,
    );

    let connectivity_failure = harness.run_runner_readme_runtime_failure(
        "host.docker.internal",
        novice_registry_only_harness_support::pick_unused_port_for_tests(),
        "runner-secret-a",
    );
    let connectivity_stderr = connectivity_failure.stderr.to_lowercase();
    assert!(
        connectivity_stderr.contains("connection refused")
            || connectivity_stderr.contains("timed out")
            || connectivity_stderr.contains("failed to lookup address information")
            || connectivity_stderr.contains("no route to host"),
        "runner README runtime must surface connectivity failures clearly; got stderr:\n{}",
        connectivity_failure.stderr,
    );
    assert!(
        !connectivity_stderr.contains("password authentication failed"),
        "connectivity failures must not masquerade as authentication failures; got stderr:\n{}",
        connectivity_failure.stderr,
    );
}

#[test]
fn verify_compose_runtime_shutdown_reports_cleanup_failures() {
    let harness = NoviceRegistryOnlyHarness::start();
    let verify_runtime = harness.start_verify_compose_runtime();
    verify_runtime.wait_until_running();

    let compose_path = harness.verify_compose_artifact_path();
    let hidden_path = compose_path.with_extension("yml.hidden");
    fs::rename(&compose_path, &hidden_path).unwrap_or_else(|error| {
        panic!(
            "verify compose artifact should be movable at `{}`: {error}",
            compose_path.display(),
        )
    });

    let shutdown_result = panic::catch_unwind(AssertUnwindSafe(|| verify_runtime.shutdown()));

    fs::rename(&hidden_path, &compose_path).unwrap_or_else(|error| {
        panic!(
            "verify compose artifact should be restorable at `{}`: {error}",
            compose_path.display(),
        )
    });

    let panic_payload = shutdown_result
        .expect_err("verify compose shutdown must fail loudly when cleanup cannot start");
    let panic_message = panic_message(panic_payload);
    assert!(
        panic_message.contains("docker compose down verify"),
        "shutdown panic must identify the cleanup command; got `{panic_message}`",
    );
    assert!(
        panic_message.contains("verify.compose.yml"),
        "shutdown panic must include the compose artifact path context; got `{panic_message}`",
    );

    verify_runtime.shutdown();
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_owned(),
            Err(_) => "non-string panic payload".to_owned(),
        },
    }
}
