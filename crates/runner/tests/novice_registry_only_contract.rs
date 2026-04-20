use std::{fs, path::PathBuf};

use serde_yaml::Value;

#[path = "support/novice_registry_only_harness.rs"]
mod novice_registry_only_harness_support;

use novice_registry_only_harness_support::NoviceRegistryOnlyHarness;

#[test]
fn copied_verify_compose_artifact_mounts_the_listener_client_ca_contract() {
    let artifact_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/compose/verify.compose.yml");
    let compose_text = fs::read_to_string(&artifact_path).unwrap_or_else(|error| {
        panic!(
            "verify compose artifact should be readable at `{}`: {error}",
            artifact_path.display(),
        )
    });
    let compose: Value = serde_yaml::from_str(&compose_text).unwrap_or_else(|error| {
        panic!(
            "verify compose artifact should stay valid yaml at `{}`: {error}",
            artifact_path.display(),
        )
    });

    let client_ca_config = compose["configs"]["verify-client-ca"]["file"]
        .as_str()
        .expect("verify compose artifact must declare the verify-client-ca config file");
    assert_eq!(
        client_ca_config, "./config/certs/client-ca.crt",
        "verify compose artifact must source the listener client CA from the copied operator workspace",
    );

    let verify_config_mounts = compose["services"]["verify"]["configs"]
        .as_sequence()
        .expect("verify compose artifact must keep the verify service configs list");
    assert!(
        verify_config_mounts.iter().any(|mount| {
            mount["source"].as_str() == Some("verify-client-ca")
                && mount["target"].as_str() == Some("/config/certs/client-ca.crt")
        }),
        "verify compose artifact must mount the listener client CA at /config/certs/client-ca.crt",
    );
}

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
        validate_output.stderr.contains("\"event\":\"config.validated\""),
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
        runner_validate_output.stderr.contains("\"event\":\"config.validated\""),
        "runner compose contract must validate config through the published image only",
    );

    let verify_runtime = harness.start_verify_compose_runtime();
    verify_runtime.wait_until_running();
}
