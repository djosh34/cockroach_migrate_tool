use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::predicate;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn validate_config_accepts_a_minimal_valid_yaml_file() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(fixture_path("valid-runner-config.yml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("config valid"))
        .stdout(predicate::str::contains("mappings=2"))
        .stdout(predicate::str::contains("verify=molt@/tmp/molt"));
}

#[test]
fn validate_config_fails_loudly_for_invalid_yaml_values() {
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(fixture_path("invalid-runner-config.yml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `reconcile.interval_secs`: must be greater than zero",
        ));
}

#[test]
fn validate_config_rejects_duplicate_mapping_ids() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
verify:
  molt:
    command: molt
    report_dir: /tmp/molt
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      connection:
        host: pg-a.example.internal
        port: 5432
        database: app_a
        user: migration_user_a
        password: runner-secret-a
  - id: app-a
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      connection:
        host: pg-b.example.internal
        port: 5432
        database: app_b
        user: migration_user_b
        password: runner-secret-b
"#,
    )
    .expect("invalid config fixture should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `mappings.id`: must be unique",
        ));
}

#[test]
fn validate_config_rejects_duplicate_source_tables_within_a_mapping() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
verify:
  molt:
    command: molt
    report_dir: /tmp/molt
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.customers
    destination:
      connection:
        host: pg-a.example.internal
        port: 5432
        database: app_a
        user: migration_user_a
        password: runner-secret-a
"#,
    )
    .expect("invalid config fixture should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `mappings.source.tables`: must not contain duplicates",
        ));
}

#[test]
fn validate_config_rejects_unqualified_source_tables() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        r#"webhook:
  bind_addr: 127.0.0.1:8443
  tls:
    cert_path: certs/server.crt
    key_path: certs/server.key
reconcile:
  interval_secs: 30
verify:
  molt:
    command: molt
    report_dir: /tmp/molt
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - customers
    destination:
      connection:
        host: pg-a.example.internal
        port: 5432
        database: app_a
        user: migration_user_a
        password: runner-secret-a
"#,
    )
    .expect("invalid config fixture should be written");

    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "config: invalid config field `mappings.source.tables`: entries must use schema.table",
        ));
}

#[test]
fn validate_config_accepts_a_mounted_config_directory_convention() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let mounted_config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&mounted_config_dir).expect("mounted config dir should be created");
    let config_path = mounted_config_dir.join("runner.yml");
    fs::copy(fixture_path("container-runner-config.yml"), &config_path)
        .expect("mounted config fixture should be written");

    let config_path_label = config_path.display().to_string();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["validate-config", "--config"])
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "config={config_path_label}"
        )))
        .stdout(predicate::str::contains("mappings=2"))
        .stdout(predicate::str::contains("verify=molt@/work/molt"))
        .stdout(predicate::str::contains(
            "tls=/config/certs/server.crt+/config/certs/server.key",
        ));
}

#[test]
fn render_postgres_setup_writes_operator_artifacts_for_each_mapping() {
    let config_path = fixture_path("valid-runner-config.yml");
    let output_dir = tempfile::tempdir().expect("temp dir should be created");
    let output_label = output_dir.path().display().to_string();
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["render-postgres-setup", "--config"])
        .arg(&config_path)
        .args(["--output-dir"])
        .arg(output_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("postgres setup artifacts written"))
        .stdout(predicate::str::contains(format!("output={output_label}")))
        .stdout(predicate::str::contains("mappings=2"));

    assert!(
        output_dir.path().join("README.md").is_file(),
        "top-level operator README should be written"
    );
    assert!(
        output_dir.path().join("app-a").join("grants.sql").is_file(),
        "first mapping grant SQL should be written"
    );
    assert!(
        output_dir.path().join("app-a").join("README.md").is_file(),
        "first mapping README should be written"
    );
    assert!(
        output_dir.path().join("app-b").join("grants.sql").is_file(),
        "second mapping grant SQL should be written"
    );
    assert!(
        output_dir.path().join("app-b").join("README.md").is_file(),
        "second mapping README should be written"
    );
}

#[test]
fn render_postgres_setup_renders_scoped_database_schema_and_table_grants() {
    let config_path = fixture_path("valid-runner-config.yml");
    let output_dir = tempfile::tempdir().expect("temp dir should be created");
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["render-postgres-setup", "--config"])
        .arg(&config_path)
        .args(["--output-dir"])
        .arg(output_dir.path())
        .assert()
        .success();

    let grants = fs::read_to_string(output_dir.path().join("app-a").join("grants.sql"))
        .expect("grant SQL should be readable");

    assert!(
        grants.contains(
            "GRANT CONNECT, TEMPORARY, CREATE ON DATABASE \"app_a\" TO \"migration_user_a\";"
        ),
        "database-level privileges should be explicit"
    );
    assert!(
        grants.contains("GRANT USAGE ON SCHEMA public TO \"migration_user_a\";"),
        "public schema usage grant should be present"
    );
    assert!(
        grants.contains(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE \"public\".\"customers\" TO \"migration_user_a\";"
        ),
        "first mapped table should receive scoped DML grants"
    );
    assert!(
        grants.contains(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE \"public\".\"orders\" TO \"migration_user_a\";"
        ),
        "second mapped table should receive scoped DML grants"
    );
    assert!(
        !grants.contains("CREATE ROLE"),
        "artifact generation must not create roles"
    );
    assert!(
        !grants.contains("SUPERUSER"),
        "artifact generation must not assume superuser privileges"
    );
}

#[test]
fn render_postgres_setup_documents_manual_grants_and_helper_schema_contract() {
    let config_path = fixture_path("valid-runner-config.yml");
    let output_dir = tempfile::tempdir().expect("temp dir should be created");
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["render-postgres-setup", "--config"])
        .arg(&config_path)
        .args(["--output-dir"])
        .arg(output_dir.path())
        .assert()
        .success();

    let top_level_readme = fs::read_to_string(output_dir.path().join("README.md"))
        .expect("top-level README should be readable");
    let mapping_readme = fs::read_to_string(output_dir.path().join("app-a").join("README.md"))
        .expect("per-mapping README should be readable");

    assert!(
        top_level_readme.contains("review them, run each `grants.sql`, then start the runner"),
        "top-level README should describe the operator sequence"
    );
    assert!(
        top_level_readme.contains("No superuser requirement is assumed or recommended."),
        "top-level README should forbid superuser assumptions"
    );
    assert!(
        mapping_readme.contains("These grants stay manual and explicit by design."),
        "per-mapping README should keep grants explicit"
    );
    assert!(
        mapping_readme.contains("No superuser requirement is assumed or recommended."),
        "per-mapping README should forbid superuser assumptions"
    );
    assert!(
        mapping_readme.contains(
            "If `_cockroach_migration_tool` already exists, it must already be owned by `migration_user_a`."
        ),
        "per-mapping README should describe the helper schema ownership contract"
    );
}

#[test]
fn render_postgres_setup_fails_loudly_when_a_mapping_artifact_path_is_not_a_directory() {
    let config_path = fixture_path("valid-runner-config.yml");
    let output_dir = tempfile::tempdir().expect("temp dir should be created");
    let blocking_path = output_dir.path().join("app-a");
    fs::write(&blocking_path, "not a directory").expect("blocking file should be written");
    let mut command = Command::cargo_bin("runner").expect("runner binary should exist");

    command
        .args(["render-postgres-setup", "--config"])
        .arg(&config_path)
        .args(["--output-dir"])
        .arg(output_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!(
            "postgres setup artifacts: failed to create mapping directory `{}`",
            blocking_path.display()
        )));
}
