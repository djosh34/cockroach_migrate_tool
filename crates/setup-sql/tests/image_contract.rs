#[path = "support/readme_contract.rs"]
mod readme_contract_support;
#[path = "support/source_bootstrap_image_contract.rs"]
mod source_bootstrap_image_contract_support;
#[path = "support/source_bootstrap_image_harness.rs"]
mod source_bootstrap_image_harness_support;

use readme_contract_support::RepositoryReadme;
use source_bootstrap_image_contract_support::SourceBootstrapImageContract;
use source_bootstrap_image_harness_support::SourceBootstrapImageHarness;
use std::{fs, path::PathBuf};

const CURSOR_PLACEHOLDER: &str = "__CHANGEFEED_CURSOR__";

#[test]
fn setup_sql_image_dockerfile_lives_in_the_setup_slice() {
    let contract = SourceBootstrapImageContract::load();

    contract.assert_setup_slice_owns_the_dockerfile();
    contract.assert_runtime_is_scratch_with_a_direct_binary_entrypoint();
}

#[test]
fn setup_sql_image_runs_emit_cockroach_sql_from_a_mounted_config() {
    let harness = SourceBootstrapImageHarness::start();
    let contract = SourceBootstrapImageContract::load();

    contract.assert_image_entrypoint_is_direct_setup_sql(&harness.image_entrypoint_json());
    harness.assert_emit_cockroach_sql_output();
}

#[test]
fn readme_setup_sql_fixture_renders_through_the_published_image_entrypoint() {
    let readme = RepositoryReadme::load();
    let harness = SourceBootstrapImageHarness::start();
    let temp_dir = fresh_temp_dir();
    let config_path = temp_dir.join("cockroach-setup.yml");
    let ca_cert_path = temp_dir.join("ca.crt");
    let fixture_text = fs::read_to_string(fixture_path("readme-cockroach-setup-config.yml"))
        .expect("README Cockroach setup fixture should be readable");

    assert_eq!(
        readme.setup_sql_cockroach_yaml_block(),
        fixture_text.trim_end(),
        "README Cockroach setup YAML should match its canonical fixture",
    );
    fs::write(&config_path, fixture_text)
        .expect("README Cockroach setup config should be writable");
    fs::write(&ca_cert_path, b"dummy-ca\n").expect("CA cert fixture should be writable");

    let output = harness.emit_cockroach_sql(&temp_dir, "/work/cockroach-setup.yml");
    assert!(
        output.starts_with("-- Source bootstrap SQL\n"),
        "published setup-sql image must render the README fixture through the container entrypoint",
    );
    assert!(
        output.contains("SELECT cluster_logical_timestamp() AS changefeed_cursor;"),
        "published setup-sql image must keep the explicit cursor capture step in the emitted SQL",
    );
    assert!(
        output.contains(&format!("cursor = '{CURSOR_PLACEHOLDER}'")),
        "published setup-sql image must emit the explicit cursor handoff in each changefeed statement",
    );
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn fresh_temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "setup-sql-readme-image-contract-{}",
        unique_suffix()
    ));
    fs::create_dir_all(&dir).expect("setup-sql README image temp dir should be created");
    dir
}

fn unique_suffix() -> String {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static UNIQUE_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos(),
        UNIQUE_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed),
    )
}
