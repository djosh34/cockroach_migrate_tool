use std::{path::PathBuf, process::Command};

use serde::Deserialize;

#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    dependencies: Vec<CargoDependency>,
}

#[derive(Deserialize)]
struct CargoDependency {
    features: Vec<String>,
    name: String,
    kind: Option<String>,
    uses_default_features: bool,
}

impl CargoDependency {
    fn is_normal(&self) -> bool {
        self.kind.is_none()
    }

    fn is_dev(&self) -> bool {
        self.kind.as_deref() == Some("dev")
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runner crate should live under workspace root")
        .parent()
        .expect("workspace root should exist")
        .to_path_buf()
}

fn cargo_metadata() -> CargoMetadata {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(workspace_root())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata command should run");

    assert!(
        output.status.success(),
        "cargo metadata should succeed, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("cargo metadata JSON should parse")
}

fn find_package<'a>(metadata: &'a CargoMetadata, package_name: &str) -> &'a CargoPackage {
    metadata
        .packages
        .iter()
        .find(|package| package.name == package_name)
        .unwrap_or_else(|| panic!("package `{package_name}` should exist in cargo metadata"))
}

fn find_normal_dependency<'a>(
    package: &'a CargoPackage,
    dependency_name: &str,
) -> &'a CargoDependency {
    package
        .dependencies
        .iter()
        .find(|dependency| dependency.is_normal() && dependency.name == dependency_name)
        .unwrap_or_else(|| {
            panic!(
                "package `{}` should keep `{dependency_name}` as a normal dependency",
                package.name
            )
        })
}

#[test]
fn operator_log_does_not_depend_on_clap() {
    let metadata = cargo_metadata();
    let operator_log = find_package(&metadata, "operator-log");
    let direct_dependencies: Vec<_> = operator_log
        .dependencies
        .iter()
        .filter(|dependency| dependency.is_normal())
        .map(|dependency| dependency.name.as_str())
        .collect();

    assert!(
        !direct_dependencies.contains(&"clap"),
        "operator-log must stay free of CLI parsing dependencies, found direct dependencies: {direct_dependencies:?}"
    );
}

#[test]
fn runner_normal_dependencies_exclude_test_harness_http_crates() {
    let metadata = cargo_metadata();
    let runner = find_package(&metadata, "runner");
    let direct_dependencies: Vec<_> = runner
        .dependencies
        .iter()
        .filter(|dependency| dependency.is_normal())
        .map(|dependency| dependency.name.as_str())
        .collect();

    for forbidden_dependency in ["http-body-util", "hyper"] {
        assert!(
            !direct_dependencies.contains(&forbidden_dependency),
            "runner normal dependencies must not include test-harness crate `{forbidden_dependency}`, found direct dependencies: {direct_dependencies:?}"
        );
    }
}

#[test]
fn runner_does_not_depend_on_clap() {
    let metadata = cargo_metadata();
    let runner = find_package(&metadata, "runner");
    let direct_dependencies: Vec<_> = runner
        .dependencies
        .iter()
        .filter(|dependency| dependency.is_normal())
        .map(|dependency| dependency.name.as_str())
        .collect();

    assert!(
        !direct_dependencies.contains(&"clap"),
        "runner must keep CLI parsing local instead of depending on clap, found direct dependencies: {direct_dependencies:?}"
    );
}

#[test]
fn runner_axum_disables_default_features() {
    let metadata = cargo_metadata();
    let runner = find_package(&metadata, "runner");
    let axum = find_normal_dependency(runner, "axum");

    assert!(
        !axum.uses_default_features,
        "runner must disable axum default features to keep the HTTP dependency surface narrow"
    );
}

#[test]
fn runner_hyper_util_does_not_enable_http2() {
    let metadata = cargo_metadata();
    let runner = find_package(&metadata, "runner");
    let hyper_util = find_normal_dependency(runner, "hyper-util");

    assert!(
        !hyper_util.features.iter().any(|feature| feature == "http2"),
        "runner should keep hyper-util on the HTTP/1-only server path, found features: {:?}",
        hyper_util.features
    );
}

#[test]
fn runner_dev_dependencies_exclude_small_encoding_helper_crates() {
    let metadata = cargo_metadata();
    let runner = find_package(&metadata, "runner");
    let direct_dev_dependencies: Vec<_> = runner
        .dependencies
        .iter()
        .filter(|dependency| dependency.is_dev())
        .map(|dependency| dependency.name.as_str())
        .collect();

    for forbidden_dependency in ["base64", "percent-encoding"] {
        assert!(
            !direct_dev_dependencies.contains(&forbidden_dependency),
            "runner test support must keep small encoding helpers local instead of depending on `{forbidden_dependency}`, found direct dev dependencies: {direct_dev_dependencies:?}"
        );
    }
}

#[test]
fn runner_dev_dependencies_exclude_cli_assertion_crates() {
    let metadata = cargo_metadata();
    let runner = find_package(&metadata, "runner");
    let direct_dev_dependencies: Vec<_> = runner
        .dependencies
        .iter()
        .filter(|dependency| dependency.is_dev())
        .map(|dependency| dependency.name.as_str())
        .collect();

    for forbidden_dependency in ["assert_cmd", "predicates"] {
        assert!(
            !direct_dev_dependencies.contains(&forbidden_dependency),
            "runner config and bootstrap contracts must keep simple command assertions local instead of depending on `{forbidden_dependency}`, found direct dev dependencies: {direct_dev_dependencies:?}"
        );
    }
}

#[test]
fn runner_config_package_keeps_validation_surface_free_of_runtime_http_and_tls_server_crates() {
    let metadata = cargo_metadata();
    let runner_config = find_package(&metadata, "runner-config");
    let direct_dependencies: Vec<_> = runner_config
        .dependencies
        .iter()
        .filter(|dependency| dependency.is_normal())
        .map(|dependency| dependency.name.as_str())
        .collect();

    for forbidden_dependency in [
        "axum",
        "hyper-util",
        "rustls",
        "rustls-pemfile",
        "serde_json",
        "tokio",
        "tokio-rustls",
    ] {
        assert!(
            !direct_dependencies.contains(&forbidden_dependency),
            "runner-config must stay focused on config validation instead of runtime server startup, found forbidden direct dependency `{forbidden_dependency}` in {direct_dependencies:?}"
        );
    }
}
