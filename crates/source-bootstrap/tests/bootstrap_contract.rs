use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::predicate;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn render_bootstrap_script_emits_a_shell_script_for_configured_mappings() {
    let mut command =
        Command::cargo_bin("source-bootstrap").expect("source-bootstrap binary should exist");

    command
        .args(["render-bootstrap-script", "--config"])
        .arg(fixture_path("valid-source-bootstrap-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::starts_with("#!/usr/bin/env bash\n"))
        .stdout(predicate::str::contains("set -euo pipefail"))
        .stdout(predicate::str::contains(
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;",
        ))
        .stdout(predicate::str::contains(
            "SELECT cluster_logical_timestamp();",
        ))
        .stdout(predicate::str::contains("# Mapping: app-a"))
        .stdout(predicate::str::contains("# Source database: demo_a"))
        .stdout(predicate::str::contains(
            "# Selected tables: public.customers, public.orders",
        ))
        .stdout(predicate::str::contains("# Mapping: app-b"))
        .stdout(predicate::str::contains(
            "CREATE CHANGEFEED FOR TABLE public.customers, public.orders",
        ))
        .stdout(predicate::str::contains(
            "CREATE CHANGEFEED FOR TABLE public.invoices",
        ))
        .stdout(predicate::str::contains(
            "WEBHOOK_BASE_URL='https://runner.example.internal:8443'",
        ))
        .stdout(predicate::str::contains(
            "INTO 'webhook-$WEBHOOK_BASE_URL/ingest/app-a'",
        ))
        .stdout(predicate::str::contains(
            "INTO 'webhook-$WEBHOOK_BASE_URL/ingest/app-b'",
        ))
        .stdout(predicate::str::contains("cursor = '$START_CURSOR'"))
        .stdout(predicate::str::contains("initial_scan = 'yes'"))
        .stdout(predicate::str::contains("envelope = 'enriched'"))
        .stdout(predicate::str::contains("enriched_properties = 'source'"))
        .stdout(predicate::str::contains("resolved = '5s'"))
        .stdout(predicate::str::contains("mapping_id=app-a"))
        .stdout(predicate::str::contains("mapping_id=app-b"))
        .stdout(predicate::str::contains("job_id=%s"));
}

#[test]
fn render_bootstrap_script_rejects_invalid_mapping_config() {
    let mut command =
        Command::cargo_bin("source-bootstrap").expect("source-bootstrap binary should exist");

    command
        .args(["render-bootstrap-script", "--config"])
        .arg(fixture_path("invalid-source-bootstrap-config.yml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "invalid config field `mappings[].id`",
        ))
        .stderr(predicate::str::contains("must be unique"));
}
