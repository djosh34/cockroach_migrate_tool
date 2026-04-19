use std::{fs, path::PathBuf};

pub struct RunnerPublicContract;

#[allow(dead_code)]
impl RunnerPublicContract {
    pub fn documented_subcommands() -> &'static [&'static str] {
        &["validate-config", "run"]
    }

    fn forbidden_removed_surface_markers() -> &'static [&'static str] {
        &[
            "compare-schema",
            "render-helper-plan",
            "render-postgres-setup",
            "verify",
            "cutover-readiness",
            "--source-url",
            "--cockroach-schema",
            "--allow-tls-mode-disable",
        ]
    }

    pub fn assert_text_excludes_removed_surface(text: &str, context: &str) {
        for marker in Self::forbidden_removed_surface_markers() {
            assert!(
                !text.contains(marker),
                "{context}: found removed runner surface marker `{marker}`",
            );
        }
    }

    pub fn assert_runtime_network_surface_contract() {
        let webhook_runtime = read_runner_source("src/webhook_runtime/mod.rs");
        assert!(
            webhook_runtime.contains("TcpListener::bind"),
            "runner runtime contract must keep the webhook listener as an explicit ingress surface",
        );

        let postgres_clients = [
            read_runner_source("src/postgres_bootstrap.rs"),
            read_runner_source("src/reconcile_runtime/mod.rs"),
            read_runner_source("src/tracking_state.rs"),
            read_runner_source("src/webhook_runtime/persistence.rs"),
        ]
        .join("\n");
        assert!(
            postgres_clients.contains("PgConnection::connect_with"),
            "runner runtime contract must keep PostgreSQL destination access explicit",
        );

        let full_runtime_text = [
            read_runner_source("src/lib.rs"),
            read_runner_source("src/postgres_bootstrap.rs"),
            read_runner_source("src/reconcile_runtime/mod.rs"),
            read_runner_source("src/runtime_plan.rs"),
            read_runner_source("src/tracking_state.rs"),
            read_runner_source("src/webhook_runtime/mod.rs"),
            read_runner_source("src/webhook_runtime/persistence.rs"),
            read_runner_source("src/webhook_runtime/routing.rs"),
        ]
        .join("\n");
        for forbidden in [
            "reqwest::",
            "hyper::Client",
            "AnyConnection",
            "MySqlConnection",
            "SqliteConnection",
            "verify_http",
            "source_url",
        ] {
            assert!(
                !full_runtime_text.contains(forbidden),
                "runner runtime contract must not regain the forbidden network client marker `{forbidden}`",
            );
        }
    }

    pub fn assert_config_surface_contract() {
        let config_model = read_runner_source("src/config/mod.rs");
        assert!(
            config_model.contains("destination: PostgresTargetConfig"),
            "runner config contract must model the destination as one explicit PostgreSQL target",
        );
        for forbidden in ["DestinationConfig", "PostgresConnectionConfig"] {
            assert!(
                !config_model.contains(forbidden),
                "runner config contract must not keep the removed generic destination wrapper `{forbidden}`",
            );
        }

        let parser = read_runner_source("src/config/parser.rs");
        assert!(
            parser.contains("mappings.destination.host"),
            "runner config parser must validate the explicit PostgreSQL destination target fields",
        );
        for forbidden in [
            "mappings.destination.connection",
            "source.connection",
            "source.url",
            "verify_http",
        ] {
            assert!(
                !parser.contains(forbidden),
                "runner config parser must not regain the forbidden surface marker `{forbidden}`",
            );
        }

        let valid_fixture = read_runner_test_file("fixtures/valid-runner-config.yml");
        assert!(
            !valid_fixture.contains("connection:"),
            "runner config examples must not reintroduce the removed destination connection wrapper",
        );
    }
}

#[allow(dead_code)]
fn read_runner_source(relative_path: &str) -> String {
    let path = runner_manifest_dir().join(relative_path);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("runner contract should read `{}`: {error}", path.display())
    })
}

#[allow(dead_code)]
fn read_runner_test_file(relative_path: &str) -> String {
    let path = runner_manifest_dir().join("tests").join(relative_path);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("runner contract should read `{}`: {error}", path.display())
    })
}

#[allow(dead_code)]
fn runner_manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
