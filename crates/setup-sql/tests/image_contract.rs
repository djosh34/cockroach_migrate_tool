#[path = "support/source_bootstrap_image_contract.rs"]
mod source_bootstrap_image_contract_support;
#[path = "support/source_bootstrap_image_harness.rs"]
mod source_bootstrap_image_harness_support;

use serde_json::Value;
use source_bootstrap_image_harness_support::SourceBootstrapImageHarness;
use std::fs;

#[test]
fn setup_sql_image_runs_emit_cockroach_sql_from_a_mounted_config() {
    let harness = SourceBootstrapImageHarness::start();

    source_bootstrap_image_contract_support::assert_image_entrypoint_is_direct_setup_sql(
        &harness.image_entrypoint_json(),
    );
    harness.assert_emit_cockroach_sql_output();
}

#[test]
fn setup_sql_image_supports_json_operator_logs_without_mixing_payload_output() {
    let harness = SourceBootstrapImageHarness::start();
    let temp_dir =
        std::env::temp_dir().join(format!("setup-sql-image-json-logs-{}", std::process::id()));
    fs::create_dir_all(&temp_dir).expect("setup-sql image json temp dir should be created");
    let config_path = temp_dir.join("cockroach-setup.yml");
    let ca_cert_path = temp_dir.join("ca.crt");
    fs::write(
        &config_path,
        fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("readme-cockroach-setup-config.yml"),
        )
        .expect("README Cockroach setup config fixture should be readable"),
    )
    .expect("temp Cockroach setup config should be writable");
    fs::write(&ca_cert_path, b"dummy-ca\n").expect("temp CA cert fixture should be writable");

    let (stdout, stderr) =
        harness.emit_cockroach_sql_json_logs(&temp_dir, "/work/cockroach-setup.yml");

    assert!(
        stdout.starts_with("-- Source bootstrap SQL\n"),
        "setup-sql image must keep stdout reserved for the SQL payload, got: {stdout:?}",
    );
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "setup-sql image json logging mode must emit exactly one log line, got: {stderr:?}",
    );

    let payload: Value =
        serde_json::from_str(lines[0]).expect("setup-sql image stderr log should be valid json");
    let json_object = payload
        .as_object()
        .expect("setup-sql image stderr log should be a json object");

    for key in ["timestamp", "level", "service", "event", "message"] {
        assert!(
            json_object.contains_key(key),
            "setup-sql image json log must include `{key}`: {payload}",
        );
    }
    assert_eq!(
        json_object.get("service").and_then(Value::as_str),
        Some("setup-sql"),
        "setup-sql image json log must identify the setup-sql service",
    );
    assert_eq!(
        json_object.get("event").and_then(Value::as_str),
        Some("sql.emitted"),
        "setup-sql image json log must expose the SQL emission event",
    );
}
