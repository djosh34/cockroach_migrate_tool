use std::{
    any::Any,
    fs,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
};

use serde_yaml::Value;

#[path = "support/novice_registry_only_harness.rs"]
mod novice_registry_only_harness_support;
#[path = "support/published_image_refs.rs"]
mod published_image_refs_support;

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
fn copied_verify_compose_artifact_uses_the_shared_bridge_network_contract() {
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

    let network_mode = compose["services"]["verify"]["network_mode"].as_str().expect(
        "verify compose artifact must declare an explicit Docker network contract for the single-service runtime",
    );
    assert_eq!(
        network_mode, "bridge",
        "verify compose artifact must reuse Docker's shared bridge network instead of allocating a project default network",
    );
}

#[test]
fn copied_setup_sql_compose_artifact_disables_project_network_allocation() {
    let artifact_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/compose/setup-sql.compose.yml");
    let compose_text = fs::read_to_string(&artifact_path).unwrap_or_else(|error| {
        panic!(
            "setup-sql compose artifact should be readable at `{}`: {error}",
            artifact_path.display(),
        )
    });
    let compose: Value = serde_yaml::from_str(&compose_text).unwrap_or_else(|error| {
        panic!(
            "setup-sql compose artifact should stay valid yaml at `{}`: {error}",
            artifact_path.display(),
        )
    });

    let network_mode = compose["services"]["setup-sql"]["network_mode"].as_str().expect(
        "setup-sql compose artifact must declare an explicit Docker network contract for the one-shot runtime",
    );
    assert_eq!(
        network_mode, "none",
        "setup-sql compose artifact must disable Docker networking instead of allocating a project default network",
    );
}

#[test]
fn copied_runner_compose_artifact_uses_the_shared_bridge_network_contract() {
    let artifact_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/compose/runner.compose.yml");
    let compose_text = fs::read_to_string(&artifact_path).unwrap_or_else(|error| {
        panic!(
            "runner compose artifact should be readable at `{}`: {error}",
            artifact_path.display(),
        )
    });
    let compose: Value = serde_yaml::from_str(&compose_text).unwrap_or_else(|error| {
        panic!(
            "runner compose artifact should stay valid yaml at `{}`: {error}",
            artifact_path.display(),
        )
    });

    let network_mode = compose["services"]["runner"]["network_mode"].as_str().expect(
        "runner compose artifact must declare an explicit Docker network contract for the single-service runtime",
    );
    assert_eq!(
        network_mode, "bridge",
        "runner compose artifact must reuse Docker's shared bridge network instead of allocating a project default network",
    );
}

#[test]
fn novice_readme_and_compose_contracts_stay_registry_only() {
    let readme_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    let readme_text = fs::read_to_string(&readme_path).unwrap_or_else(|error| {
        panic!(
            "README should be readable at `{}`: {error}",
            readme_path.display(),
        )
    });
    let novice_surface = readme_text
        .split("## Setup SQL Quick Start")
        .nth(1)
        .and_then(|remainder| remainder.split("## CI Publish Safety").next())
        .expect("README must keep the novice-user quick start surface grouped together");

    for forbidden in [
        "git clone",
        "docker build",
        "cargo ",
        "cargo\n",
        "make ",
        "make\n",
        "AGENTS.md",
        "CONTRIBUTING.md",
    ] {
        assert!(
            !novice_surface.contains(forbidden),
            "novice README surface must stay registry-only and repo-free; found forbidden snippet `{forbidden}`",
        );
    }

    for (artifact_name, env_var) in [
        ("setup-sql.compose.yml", "${SETUP_SQL_IMAGE}"),
        ("runner.compose.yml", "${RUNNER_IMAGE}"),
        ("verify.compose.yml", "${VERIFY_IMAGE}"),
    ] {
        let artifact_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/compose")
            .join(artifact_name);
        let artifact_text = fs::read_to_string(&artifact_path).unwrap_or_else(|error| {
            panic!(
                "compose artifact should be readable at `{}`: {error}",
                artifact_path.display(),
            )
        });
        assert!(
            artifact_text.contains(env_var),
            "compose artifact `{artifact_name}` must keep using the published image env var `{env_var}`",
        );
        assert!(
            !artifact_text.contains("\nbuild:"),
            "compose artifact `{artifact_name}` must not require a local docker build",
        );
    }
}

#[test]
fn readme_public_image_quick_start_documents_secure_runner_and_verify_config_inline() {
    let readme_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    let readme_text = fs::read_to_string(&readme_path).unwrap_or_else(|error| {
        panic!(
            "README should be readable at `{}`: {error}",
            readme_path.display(),
        )
    });

    for required_snippet in [
        "# config/runner.yml",
        "destination:\n      host: pg-a.example.internal",
        "tls:\n        mode: verify-ca",
        "ca_cert_path: /config/certs/destination-ca.crt",
        "client_cert_path: /config/certs/destination-client.crt",
        "client_key_path: /config/certs/destination-client.key",
        "# config/verify-service.yml",
        "client_ca_path: /config/certs/client-ca.crt",
        "- source: verify-client-ca",
        "file: ./config/certs/client-ca.crt",
    ] {
        assert!(
            readme_text.contains(required_snippet),
            "README public-image quick start must document secure runner and verify config inline; missing `{required_snippet}`",
        );
    }
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

    let shutdown_result =
        panic::catch_unwind(AssertUnwindSafe(|| verify_runtime.shutdown()));

    fs::rename(&hidden_path, &compose_path).unwrap_or_else(|error| {
        panic!(
            "verify compose artifact should be restorable at `{}`: {error}",
            compose_path.display(),
        )
    });

    let panic_payload =
        shutdown_result.expect_err("verify compose shutdown must fail loudly when cleanup cannot start");
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
