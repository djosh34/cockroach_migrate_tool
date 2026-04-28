use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::docker_image_container_harness_support::DockerImageContainer;
use crate::nix_image_artifact_harness_support::{
    NixImageArtifact, run_command_capture, run_command_output,
};

pub struct VerifyImageArtifactHarness {
    image_tag: String,
}

impl VerifyImageArtifactHarness {
    pub fn start() -> Self {
        let harness = Self {
            image_tag: format!("cockroach-migrate-verify-test-{}", unique_suffix()),
        };
        harness.build_verify_image();
        harness
    }

    pub fn assert_image_exists(&self) {
        run_command_capture(
            Command::new("docker").args([
                "image",
                "inspect",
                &self.image_tag,
                "--format",
                "{{.Id}}",
            ]),
            "docker image inspect verify image",
        );
    }

    pub fn validate_config_json_logs(&self) -> (String, String) {
        let fixture_mount = format!(
            "{}:/work/testdata:ro",
            verify_slice_root()
                .join("verifyservice")
                .join("testdata")
                .display()
        );
        run_command_output(
            Command::new("docker").args([
                "run",
                "--rm",
                "--entrypoint",
                "/usr/local/bin/molt",
                "-v",
                &fixture_mount,
                &self.image_tag,
                "verify-service",
                "validate-config",
                "--log-format",
                "json",
                "--config",
                "/work/testdata/valid-https-mtls.yml",
            ]),
            "docker run verify image validate-config --log-format json",
        )
    }

    pub fn assert_embedded_module_meets_minimum_version(
        &self,
        module: &str,
        minimum_version: &str,
    ) {
        let current_version = self
            .embedded_module_version(module)
            .unwrap_or_else(|| panic!("verify image must embed module `{module}`"));
        let current_version = parse_go_semver(&current_version).unwrap_or_else(|| {
            panic!(
                "embedded module `{module}` should use a semver version, found `{current_version}`"
            )
        });
        let minimum_version = parse_go_semver(minimum_version).unwrap_or_else(|| {
            panic!("minimum version floor should be valid semver, found `{minimum_version}`")
        });

        assert!(
            current_version >= minimum_version,
            "verify image must embed `{module}` at or above `{}`, found `{}`",
            format_go_semver(&minimum_version),
            format_go_semver(&current_version),
        );
    }

    pub fn assert_embedded_module_is_absent_or_meets_minimum_version(
        &self,
        module: &str,
        minimum_version: &str,
    ) {
        let Some(current_version) = self.embedded_module_version(module) else {
            return;
        };
        let current_version = parse_go_semver(&current_version).unwrap_or_else(|| {
            panic!(
                "embedded module `{module}` should use a semver version, found `{current_version}`"
            )
        });
        let minimum_version = parse_go_semver(minimum_version).unwrap_or_else(|| {
            panic!("minimum version floor should be valid semver, found `{minimum_version}`")
        });

        assert!(
            current_version >= minimum_version,
            "verify image must either omit `{module}` or embed it at or above `{}`, found `{}`",
            format_go_semver(&minimum_version),
            format_go_semver(&current_version),
        );
    }

    fn build_verify_image(&self) {
        NixImageArtifact::verify().provision_image_tag(&self.image_tag, "verify image");
    }

    fn embedded_module_version(&self, module: &str) -> Option<String> {
        self.molt_binary_go_version_metadata()
            .lines()
            .filter_map(parse_embedded_module_line)
            .find_map(|embedded_module| {
                (embedded_module.module == module).then_some(embedded_module.version)
            })
    }

    fn molt_binary_go_version_metadata(&self) -> String {
        let extracted_binary = ExtractedVerifyBinary::from_image(&self.image_tag);

        run_command_capture(
            Command::new("go").args(["version", "-m", extracted_binary.binary_path()]),
            "go version -m extracted verify binary",
        )
    }
}

impl Drop for VerifyImageArtifactHarness {
    fn drop(&mut self) {
        let output = Command::new("docker")
            .args(["image", "inspect", &self.image_tag])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker image inspect verify image should start: {error}")
            });
        if output.status.success() {
            run_command_capture(
                Command::new("docker").args(["rmi", "-f", &self.image_tag]),
                "docker rmi verify image",
            );
        }
    }
}

fn unique_suffix() -> String {
    static UNIQUE_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos(),
        UNIQUE_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

struct ExtractedVerifyBinary {
    temp_dir: PathBuf,
    binary_path: PathBuf,
}

impl ExtractedVerifyBinary {
    fn from_image(image_tag: &str) -> Self {
        let temp_dir =
            std::env::temp_dir().join(format!("verify-image-artifact-{}", unique_suffix()));
        fs::create_dir_all(&temp_dir).unwrap_or_else(|error| {
            panic!(
                "temporary directory `{}` should be creatable: {error}",
                temp_dir.display()
            )
        });

        let binary_path = temp_dir.join("molt");
        let container =
            DockerImageContainer::create(image_tag, "verify image extraction container");
        container.copy_file(
            "/usr/local/bin/molt",
            &binary_path,
            "docker cp verify binary from image",
        );

        Self {
            temp_dir,
            binary_path,
        }
    }

    fn binary_path(&self) -> &str {
        self.binary_path
            .to_str()
            .expect("temporary binary path should be valid utf-8")
    }
}

impl Drop for ExtractedVerifyBinary {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.temp_dir).unwrap_or_else(|error| {
            panic!(
                "temporary directory `{}` should be removable: {error}",
                self.temp_dir.display()
            )
        });
    }
}

struct EmbeddedModule {
    module: String,
    version: String,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct GoSemver {
    major: u64,
    minor: u64,
    patch: u64,
    pre_release: bool,
}

fn verify_slice_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("cockroachdb_molt/molt")
}

fn parse_embedded_module_line(line: &str) -> Option<EmbeddedModule> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "dep" {
        return None;
    }

    Some(EmbeddedModule {
        module: parts.next()?.to_owned(),
        version: parts.next()?.to_owned(),
    })
}

fn parse_go_semver(version: &str) -> Option<GoSemver> {
    let normalized = version.strip_prefix('v')?;
    if normalized.contains('+') {
        return None;
    }
    let (core, suffix) = normalized.split_once('-').unwrap_or((normalized, ""));
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    Some(GoSemver {
        major,
        minor,
        patch,
        pre_release: !suffix.is_empty(),
    })
}

fn format_go_semver(version: &GoSemver) -> String {
    format!("v{}.{}.{}", version.major, version.minor, version.patch)
}
