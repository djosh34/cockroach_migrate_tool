use std::{
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use predicates::prelude::predicate;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn write_script(path: &Path, contents: &str) {
    fs::write(path, contents).expect("script fixture should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)
            .expect("script fixture metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("script fixture should be executable");
    }
}

fn write_verify_config(path: &Path, report_dir: &Path, molt_command: &Path) {
    let config = format!(
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: 30
verify:
  molt:
    command: {molt_command}
    report_dir: {report_dir}
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
    destination:
      connection:
        host: pg-a.example.internal
        port: 5432
        database: app_a
        user: migration_user_a
        password: runner-secret-a
"#,
        cert_path = fixture_path("certs/server.crt").display(),
        key_path = fixture_path("certs/server.key").display(),
        molt_command = molt_command.display(),
        report_dir = report_dir.display(),
    );

    fs::write(path, config).expect("verify config should be written");
}

fn write_clean_verify_script(path: &Path) {
    write_script(
        path,
        r#"#!/usr/bin/env bash
set -euo pipefail

cat <<'EOF'
{"type":"summary","table_schema":"public","table_name":"customers","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0,"num_success":24,"num_truth_rows":24}
{"type":"summary","table_schema":"public","table_name":"orders","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0,"num_success":72,"num_truth_rows":72}
{"level":"info","message":"verification complete"}
EOF
"#,
    );
}

#[test]
fn verify_succeeds_for_a_clean_molt_summary() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_clean_verify_script(&script_path);
    write_verify_config(&config_path, &report_dir, &script_path);

    let artifact_label = report_dir.display().to_string();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["verify", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .success()
        .stdout(predicate::str::contains("verification"))
        .stdout(predicate::str::contains("mapping=app-a"))
        .stdout(predicate::str::contains("verdict=matched"))
        .stdout(predicate::str::contains(format!(
            "artifacts={artifact_label}"
        )));
}

#[test]
fn verify_fails_when_molt_reports_mismatch_counters_with_exit_code_zero() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        r#"#!/usr/bin/env bash
set -euo pipefail

cat <<'EOF'
{"type":"data","table_schema":"public","table_name":"customers","message":"mismatching row value"}
{"type":"summary","table_schema":"public","table_name":"customers","num_missing":0,"num_mismatch":1,"num_extraneous":0,"num_column_mismatch":0,"num_success":23,"num_truth_rows":24}
{"type":"summary","table_schema":"public","table_name":"orders","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0,"num_success":72,"num_truth_rows":72}
{"level":"info","message":"verification complete"}
EOF
"#,
    );
    write_verify_config(&config_path, &report_dir, &script_path);

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["verify", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "verify: data mismatches detected for mapping `app-a`",
        ))
        .stderr(predicate::str::contains("customers"))
        .stderr(predicate::str::contains("num_mismatch=1"));
}

#[test]
fn verify_uses_real_mapping_tables_and_writes_report_artifacts() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let args_path = temp_dir.path().join("molt-args.txt");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' "$@" > '{}'

cat <<'EOF'
{{"type":"summary","table_schema":"public","table_name":"customers","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0,"num_success":24,"num_truth_rows":24}}
{{"type":"summary","table_schema":"public","table_name":"orders","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0,"num_success":72,"num_truth_rows":72}}
{{"level":"info","message":"verification complete"}}
EOF
"#,
            args_path.display()
        ),
    );
    write_verify_config(&config_path, &report_dir, &script_path);

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["verify", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "artifacts={}",
            report_dir.display()
        )));

    let captured_args = fs::read_to_string(&args_path).expect("captured args should exist");
    assert!(
        captured_args.contains("--schema-filter\npublic\n"),
        "schema filter should use the real mapped schema: {captured_args}"
    );
    assert!(
        captured_args.contains("--table-filter\ncustomers|orders\n"),
        "table filter should use only the real mapped tables: {captured_args}"
    );
    assert!(
        !captured_args.contains("_cockroach_migration_tool"),
        "helper tables must never be passed to molt verify: {captured_args}"
    );
    assert!(
        report_dir.join("app-a.raw.log").is_file(),
        "raw log artifact should be written"
    );
    assert!(
        report_dir.join("app-a.summary.json").is_file(),
        "machine-readable summary artifact should be written"
    );
}

#[test]
fn verify_fails_loudly_for_incomplete_output_and_still_writes_raw_log() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        r#"#!/usr/bin/env bash
set -euo pipefail

cat <<'EOF'
non-json prelude
{"level":"info","message":"starting verify"}
EOF
"#,
    );
    write_verify_config(&config_path, &report_dir, &script_path);

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["verify", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "verify: molt verify for mapping `app-a` did not emit any summary records",
        ));

    let raw_log_path = report_dir.join("app-a.raw.log");
    assert!(
        raw_log_path.is_file(),
        "raw log should be preserved on failure"
    );
    let raw_log = fs::read_to_string(raw_log_path).expect("raw log should be readable");
    assert!(
        raw_log.contains("non-json prelude"),
        "raw failure log should contain the original molt output"
    );
}

#[test]
fn verify_passes_through_allow_tls_mode_disable_when_requested() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let args_path = temp_dir.path().join("molt-args.txt");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' "$@" > '{}'

cat <<'EOF'
{{"type":"summary","table_schema":"public","table_name":"customers","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0,"num_success":24,"num_truth_rows":24}}
{{"type":"summary","table_schema":"public","table_name":"orders","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0,"num_success":72,"num_truth_rows":72}}
{{"level":"info","message":"verification complete"}}
EOF
"#,
            args_path.display()
        ),
    );
    write_verify_config(&config_path, &report_dir, &script_path);

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["verify", "--config"])
        .arg(&config_path)
        .args([
            "--mapping",
            "app-a",
            "--source-url",
            "postgres://root@127.0.0.1:26257/demo_a?sslmode=disable",
            "--allow-tls-mode-disable",
        ])
        .assert()
        .success();

    let captured_args = fs::read_to_string(&args_path).expect("captured args should exist");
    assert!(
        captured_args.contains("--allow-tls-mode-disable"),
        "the explicit tls-disable passthrough flag should reach molt: {captured_args}"
    );
}
