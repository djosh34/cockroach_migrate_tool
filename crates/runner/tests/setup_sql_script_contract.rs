use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("setup_sql_scripts")
        .join(name)
}

fn percent_encoded_ca_cert(path: &Path) -> String {
    utf8_percent_encode(
        &STANDARD.encode(fs::read(path).expect("ca cert should be readable")),
        NON_ALPHANUMERIC,
    )
    .to_string()
}

fn expected_fixture(name: &str) -> String {
    let fixture = fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|error| panic!("expected fixture `{name}` should be readable: {error}"));

    fixture.replace(
        "__CA_CERT_BASE64__",
        &percent_encoded_ca_cert(&fixture_path("ca.crt")),
    )
}

fn run_script(script_name: &str, args: &[&str]) -> Output {
    let script_path = repo_root().join("scripts").join(script_name);
    Command::new(&script_path)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("{script_name} should start: {error}"))
}

fn find_command(command_name: &str) -> PathBuf {
    for path_entry in std::env::var_os("PATH")
        .expect("PATH should exist")
        .to_string_lossy()
        .split(':')
    {
        let candidate = PathBuf::from(path_entry).join(command_name);
        if candidate.is_file() {
            return candidate;
        }
    }

    panic!("{command_name} should exist for fallback test");
}

fn path_without_yq() -> tempfile::TempDir {
    let bin_dir = tempfile::tempdir().expect("bin dir should be created");
    for command_name in [
        "bash", "base64", "dirname", "envsubst", "mkdir", "python3", "sort",
    ] {
        let target = find_command(command_name);
        symlink(target, bin_dir.path().join(command_name))
            .unwrap_or_else(|error| panic!("symlink for {command_name} should be created: {error}"));
    }
    bin_dir
}

#[test]
fn generate_cockroach_setup_sql_renders_expected_sql_for_a_single_database() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let config_path = fixture_path("cockroach-setup-config.yml");
    let expected = expected_fixture("cockroach-demo_a-setup.expected.sql");

    let output = run_script(
        "generate-cockroach-setup-sql.sh",
        &[
            "--output-dir",
            output_dir
                .path()
                .to_str()
                .expect("output dir should be utf-8"),
            config_path
                .to_str()
                .expect("cockroach config path should be utf-8"),
        ],
    );

    assert!(
        output.status.success(),
        "cockroach setup sql script failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let per_database_sql = fs::read_to_string(output_dir.path().join("cockroach-demo_a-setup.sql"))
        .expect("per-database sql file should be written");
    let combined_sql = fs::read_to_string(output_dir.path().join("cockroach-all-setup.sql"))
        .expect("combined sql file should be written");

    assert_eq!(per_database_sql, expected);
    assert_eq!(combined_sql, expected);
}

#[test]
fn generate_cockroach_setup_sql_merges_mappings_by_database_and_concatenates_combined_output() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let config_path = fixture_path("cockroach-multi-database-config.yml");
    let output = run_script(
        "generate-cockroach-setup-sql.sh",
        &[
            "--output-dir",
            output_dir
                .path()
                .to_str()
                .expect("output dir should be utf-8"),
            config_path
                .to_str()
                .expect("cockroach multi config path should be utf-8"),
        ],
    );

    assert!(
        output.status.success(),
        "cockroach setup sql script failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let demo_a_sql = fs::read_to_string(output_dir.path().join("cockroach-demo_a-setup.sql"))
        .expect("demo_a sql file should be written");
    let demo_b_sql = fs::read_to_string(output_dir.path().join("cockroach-demo_b-setup.sql"))
        .expect("demo_b sql file should be written");
    let combined_sql = fs::read_to_string(output_dir.path().join("cockroach-all-setup.sql"))
        .expect("combined sql file should be written");

    assert_eq!(demo_a_sql, expected_fixture("cockroach-demo_a-merged.expected.sql"));
    assert_eq!(demo_b_sql, expected_fixture("cockroach-demo_b-setup.expected.sql"));
    assert_eq!(combined_sql, expected_fixture("cockroach-all-setup.expected.sql"));
}

#[test]
fn generate_cockroach_setup_sql_dry_run_prints_without_writing_files() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let config_path = fixture_path("cockroach-setup-config.yml");
    let output = run_script(
        "generate-cockroach-setup-sql.sh",
        &[
            "--dry-run",
            "--output-dir",
            output_dir
                .path()
                .to_str()
                .expect("output dir should be utf-8"),
            config_path
                .to_str()
                .expect("cockroach config path should be utf-8"),
        ],
    );

    assert!(
        output.status.success(),
        "cockroach dry run failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("cockroach-demo_a-setup.sql"),
        "dry run stdout should name the per-database file"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("cockroach-all-setup.sql"),
        "dry run stdout should name the combined file"
    );
    assert!(
        fs::read_dir(output_dir.path())
            .expect("output dir should be readable")
            .next()
            .is_none(),
        "dry run should not write any files"
    );
}

#[test]
fn generate_cockroach_setup_sql_help_prints_usage() {
    let output = run_script("generate-cockroach-setup-sql.sh", &["--help"]);

    assert!(
        output.status.success(),
        "cockroach help failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Usage: ./scripts/generate-cockroach-setup-sql.sh"),
        "help stdout should contain usage"
    );
}

#[test]
fn generate_cockroach_setup_sql_reports_missing_required_keys() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let config_path = fixture_path("invalid-cockroach-config.yml");
    let output = run_script(
        "generate-cockroach-setup-sql.sh",
        &[
            "--output-dir",
            output_dir
                .path()
                .to_str()
                .expect("output dir should be utf-8"),
            config_path
                .to_str()
                .expect("invalid cockroach config path should be utf-8"),
        ],
    );

    assert!(
        !output.status.success(),
        "invalid cockroach config should fail:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing required key: webhook.base_url"),
        "stderr should explain the missing key"
    );
}

#[test]
fn generate_cockroach_setup_sql_falls_back_to_python3_when_yq_is_unavailable() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let path_dir = path_without_yq();
    let script_path = repo_root().join("scripts/generate-cockroach-setup-sql.sh");
    let config_path = fixture_path("cockroach-setup-config.yml");

    let output = Command::new(&script_path)
        .env("PATH", path_dir.path())
        .arg("--output-dir")
        .arg(output_dir.path())
        .arg(&config_path)
        .output()
        .unwrap_or_else(|error| panic!("cockroach fallback script should start: {error}"));

    assert!(
        output.status.success(),
        "cockroach fallback failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(output_dir.path().join("cockroach-demo_a-setup.sql"))
            .expect("fallback cockroach output should be written"),
        expected_fixture("cockroach-demo_a-setup.expected.sql")
    );
}

#[test]
fn generate_postgres_grants_sql_renders_expected_sql_for_a_single_database() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let config_path = fixture_path("postgres-grants-config.yml");
    let output = run_script(
        "generate-postgres-grants-sql.sh",
        &[
            "--output-dir",
            output_dir
                .path()
                .to_str()
                .expect("output dir should be utf-8"),
            config_path
                .to_str()
                .expect("postgres config path should be utf-8"),
        ],
    );

    assert!(
        output.status.success(),
        "postgres grants script failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let per_database_sql = fs::read_to_string(output_dir.path().join("postgres-app_a-grants.sql"))
        .expect("per-database postgres sql file should be written");
    let combined_sql = fs::read_to_string(output_dir.path().join("postgres-all-grants.sql"))
        .expect("combined postgres sql file should be written");
    let expected = expected_fixture("postgres-app_a-grants.expected.sql");

    assert_eq!(per_database_sql, expected);
    assert_eq!(combined_sql, expected);
}

#[test]
fn generate_postgres_grants_sql_deduplicates_grants_for_shared_database_and_role() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let config_path = fixture_path("postgres-dedup-config.yml");
    let output = run_script(
        "generate-postgres-grants-sql.sh",
        &[
            "--output-dir",
            output_dir
                .path()
                .to_str()
                .expect("output dir should be utf-8"),
            config_path
                .to_str()
                .expect("postgres dedupe config path should be utf-8"),
        ],
    );

    assert!(
        output.status.success(),
        "postgres grants dedupe script failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let per_database_sql = fs::read_to_string(output_dir.path().join("postgres-app_a-grants.sql"))
        .expect("per-database postgres sql file should be written");
    let combined_sql = fs::read_to_string(output_dir.path().join("postgres-all-grants.sql"))
        .expect("combined postgres sql file should be written");
    let expected = expected_fixture("postgres-app_a-deduped.expected.sql");

    assert_eq!(per_database_sql, expected);
    assert_eq!(combined_sql, expected);
}

#[test]
fn generate_postgres_grants_sql_dry_run_prints_without_writing_files() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let config_path = fixture_path("postgres-grants-config.yml");
    let output = run_script(
        "generate-postgres-grants-sql.sh",
        &[
            "--dry-run",
            "--output-dir",
            output_dir
                .path()
                .to_str()
                .expect("output dir should be utf-8"),
            config_path
                .to_str()
                .expect("postgres config path should be utf-8"),
        ],
    );

    assert!(
        output.status.success(),
        "postgres dry run failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("postgres-app_a-grants.sql"),
        "dry run stdout should name the per-database file"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("postgres-all-grants.sql"),
        "dry run stdout should name the combined file"
    );
    assert!(
        fs::read_dir(output_dir.path())
            .expect("output dir should be readable")
            .next()
            .is_none(),
        "dry run should not write any files"
    );
}

#[test]
fn generate_postgres_grants_sql_help_prints_usage() {
    let output = run_script("generate-postgres-grants-sql.sh", &["--help"]);

    assert!(
        output.status.success(),
        "postgres help failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Usage: ./scripts/generate-postgres-grants-sql.sh"),
        "help stdout should contain usage"
    );
}

#[test]
fn generate_postgres_grants_sql_reports_missing_required_keys() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let config_path = fixture_path("invalid-postgres-config.yml");
    let output = run_script(
        "generate-postgres-grants-sql.sh",
        &[
            "--output-dir",
            output_dir
                .path()
                .to_str()
                .expect("output dir should be utf-8"),
            config_path
                .to_str()
                .expect("invalid postgres config path should be utf-8"),
        ],
    );

    assert!(
        !output.status.success(),
        "invalid postgres config should fail:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("missing required key: mappings[0].destination.runtime_role"),
        "stderr should explain the missing key"
    );
}

#[test]
fn generate_postgres_grants_sql_falls_back_to_python3_when_yq_is_unavailable() {
    let output_dir = tempfile::tempdir().expect("output dir should be created");
    let path_dir = path_without_yq();
    let script_path = repo_root().join("scripts/generate-postgres-grants-sql.sh");
    let config_path = fixture_path("postgres-grants-config.yml");

    let output = Command::new(&script_path)
        .env("PATH", path_dir.path())
        .arg("--output-dir")
        .arg(output_dir.path())
        .arg(&config_path)
        .output()
        .unwrap_or_else(|error| panic!("postgres fallback script should start: {error}"));

    assert!(
        output.status.success(),
        "postgres fallback failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(output_dir.path().join("postgres-app_a-grants.sql"))
            .expect("fallback postgres output should be written"),
        expected_fixture("postgres-app_a-grants.expected.sql")
    );
}
