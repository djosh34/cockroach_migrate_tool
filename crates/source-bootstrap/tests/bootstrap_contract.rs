use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "support/readme_contract.rs"]
mod readme_contract_support;

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};
use readme_contract_support::RepositoryReadme;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn fresh_temp_dir() -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "source-bootstrap-readme-contract-{}-{unique_suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

const FIXTURE_CA_CERT_QUERY: &str = "ZHVtbXktY2EK";

#[test]
fn render_bootstrap_sql_emits_sql_only_output_for_configured_mappings() {
    let mut command =
        Command::cargo_bin("source-bootstrap").expect("source-bootstrap binary should exist");

    command
        .args(["render-bootstrap-sql", "--config"])
        .arg(fixture_path("valid-source-bootstrap-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::starts_with("-- Source bootstrap SQL\n"))
        .stdout(predicate::str::contains(
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;",
        ))
        .stdout(predicate::str::contains(
            "SELECT cluster_logical_timestamp();",
        ))
        .stdout(predicate::str::contains("-- Mapping: app-a"))
        .stdout(predicate::str::contains("-- Source database: demo_a"))
        .stdout(predicate::str::contains(
            "-- Selected tables: public.customers, public.orders",
        ))
        .stdout(predicate::str::contains("-- Mapping: app-b"))
        .stdout(predicate::str::contains(
            "CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders",
        ))
        .stdout(predicate::str::contains(
            "CREATE CHANGEFEED FOR TABLE demo_b.public.invoices",
        ))
        .stdout(predicate::str::contains(format!(
            "INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert={FIXTURE_CA_CERT_QUERY}'"
        )))
        .stdout(predicate::str::contains(format!(
            "INTO 'webhook-https://runner.example.internal:8443/ingest/app-b?ca_cert={FIXTURE_CA_CERT_QUERY}'"
        )))
        .stdout(predicate::str::contains("initial_scan = 'yes'"))
        .stdout(predicate::str::contains("envelope = 'enriched'"))
        .stdout(predicate::str::contains("enriched_properties = 'source'"))
        .stdout(predicate::str::contains("resolved = '5s'"))
        .stdout(predicate::str::contains("#!/usr/bin/env bash").not())
        .stdout(predicate::str::contains("set -euo pipefail").not())
        .stdout(predicate::str::contains("COCKROACH_URL=").not())
        .stdout(predicate::str::contains("WEBHOOK_BASE_URL=").not())
        .stdout(predicate::str::contains("START_CURSOR=").not())
        .stdout(predicate::str::contains("tail -n +2").not())
        .stdout(predicate::str::contains("cut -d, -f1").not())
        .stdout(predicate::str::contains("printf '").not());
}

#[test]
fn render_bootstrap_sql_rejects_invalid_mapping_config() {
    let mut command =
        Command::cargo_bin("source-bootstrap").expect("source-bootstrap binary should exist");

    command
        .args(["render-bootstrap-sql", "--config"])
        .arg(fixture_path("invalid-source-bootstrap-config.yml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "invalid config field `mappings[].id`",
        ))
        .stderr(predicate::str::contains("must be unique"));
}

#[test]
fn readme_source_bootstrap_config_is_copyable_by_the_public_cli() {
    let readme = RepositoryReadme::load();
    let temp_dir = fresh_temp_dir();
    let config_path = temp_dir.join("source-bootstrap.yml");
    let ca_cert_path = temp_dir.join("ca.crt");
    let fixture_text = fs::read_to_string(fixture_path("readme-source-bootstrap-config.yml"))
        .expect("README source bootstrap fixture should be readable");
    assert_eq!(
        readme.source_bootstrap_yaml_block(),
        fixture_text.trim_end(),
        "README source bootstrap YAML should match its canonical fixture"
    );
    fs::write(&config_path, fixture_text)
        .expect("README source bootstrap config should be writable");
    fs::write(&ca_cert_path, b"dummy-ca\n").expect("CA cert fixture should be writable");

    let mut command =
        Command::cargo_bin("source-bootstrap").expect("source-bootstrap binary should exist");

    command
        .args(["render-bootstrap-sql", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("-- Source bootstrap SQL\n"));
}
