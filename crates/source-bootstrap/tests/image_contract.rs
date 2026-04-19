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

#[test]
fn source_bootstrap_image_dockerfile_lives_in_the_source_bootstrap_slice() {
    let contract = SourceBootstrapImageContract::load();

    contract.assert_source_bootstrap_slice_owns_the_dockerfile();
    contract.assert_runtime_is_scratch_with_a_direct_binary_entrypoint();
}

#[test]
fn source_bootstrap_image_runs_render_bootstrap_sql_from_a_mounted_config() {
    let harness = SourceBootstrapImageHarness::start();
    let contract = SourceBootstrapImageContract::load();

    contract.assert_image_entrypoint_is_direct_source_bootstrap(&harness.image_entrypoint_json());
    harness.assert_render_bootstrap_sql_output();
}

#[test]
fn readme_source_bootstrap_fixture_renders_through_the_published_image_entrypoint() {
    let readme = RepositoryReadme::load();
    let harness = SourceBootstrapImageHarness::start();
    let temp_dir = fresh_temp_dir();
    let config_path = temp_dir.join("source-bootstrap.yml");
    let ca_cert_path = temp_dir.join("ca.crt");
    let fixture_text = fs::read_to_string(fixture_path("readme-source-bootstrap-config.yml"))
        .expect("README source bootstrap fixture should be readable");

    assert_eq!(
        readme.source_bootstrap_yaml_block(),
        fixture_text.trim_end(),
        "README source bootstrap YAML should match its canonical fixture",
    );
    fs::write(&config_path, fixture_text)
        .expect("README source bootstrap config should be writable");
    fs::write(&ca_cert_path, b"dummy-ca\n").expect("CA cert fixture should be writable");

    let output = harness.render_bootstrap_sql(&temp_dir, "/work/source-bootstrap.yml");
    assert!(
        output.starts_with("-- Source bootstrap SQL\n"),
        "published source-bootstrap image must render the README fixture through the container entrypoint",
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
        "source-bootstrap-readme-image-contract-{}",
        unique_suffix()
    ));
    fs::create_dir_all(&dir).expect("source-bootstrap README image temp dir should be created");
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
