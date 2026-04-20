use std::{fs, path::PathBuf};

use serde_yaml::{Mapping, Value};

use crate::image_build_target_contract_support::ImageBuildTargetContract;
use crate::published_image_contract_support::PublishedImageContract;

pub struct GithubWorkflowContract {
    workflow_text: String,
    readme_text: String,
    document: Value,
}

const FAST_VALIDATION_JOB_NAME: &str = "validate-fast";
const LONG_VALIDATION_JOB_NAME: &str = "validate-long";
const QUAY_SECURITY_GATE_JOB_NAME: &str = "quay-security-gate";
const FAST_VALIDATION_COMMANDS: [&str; 3] = ["make check", "make lint", "make test"];
const LONG_VALIDATION_COMMANDS: [&str; 1] = ["make test-long"];
const GHCR_REGISTRY_HOST: &str = "ghcr.io";
const QUAY_REGISTRY_HOST: &str = "quay.io";
const QUAY_NAMESPACE: &str = "djosh34";

impl GithubWorkflowContract {
    pub fn load_publish_images() -> Self {
        let workflow_path = repo_root().join(".github/workflows/publish-images.yml");
        let readme_path = repo_root().join("README.md");
        let workflow_text = fs::read_to_string(&workflow_path).unwrap_or_else(|error| {
            panic!(
                "publish images workflow `{}` should be readable: {error}",
                workflow_path.display()
            )
        });
        let readme_text = fs::read_to_string(&readme_path).unwrap_or_else(|error| {
            panic!(
                "README `{}` should be readable: {error}",
                readme_path.display()
            )
        });
        let document = serde_yaml::from_str(&workflow_text).unwrap_or_else(|error| {
            panic!(
                "publish images workflow `{}` should parse as YAML: {error}",
                workflow_path.display()
            )
        });

        Self {
            workflow_text,
            readme_text,
            document,
        }
    }

    pub fn assert_pushes_to_main_only(&self) {
        assert_eq!(
            self.push_branches(),
            vec!["main"],
            "workflow must trigger only on pushes to `main`",
        );
        assert!(
            !self
                .workflow_on()
                .contains_key(Value::String("pull_request".to_owned())),
            "workflow must not define a pull_request trigger",
        );
    }

    pub fn assert_rejects_outsider_controlled_and_drift_prone_triggers(&self) {
        assert_eq!(
            self.push_branches(),
            vec!["main"],
            "workflow must trigger only on pushes to `main`",
        );
        assert!(
            self.push_tags().is_none(),
            "workflow must not trigger on tag pushes",
        );

        for forbidden_trigger in [
            "pull_request",
            "pull_request_target",
            "workflow_dispatch",
            "workflow_run",
            "workflow_call",
            "schedule",
            "release",
            "issues",
            "issue_comment",
        ] {
            assert!(
                !self
                    .workflow_on()
                    .contains_key(Value::String(forbidden_trigger.to_owned())),
                "workflow must not define a `{forbidden_trigger}` trigger",
            );
        }
    }

    pub fn assert_keeps_publish_permissions_and_credentials_out_of_validation(&self) {
        let workflow_permissions = self
            .workflow_permissions()
            .expect("workflow should define top-level permissions");
        assert_eq!(
            workflow_permissions
                .get(Value::String("contents".to_owned()))
                .map(value_as_str),
            Some("read"),
            "workflow should keep top-level permissions read-only",
        );
        assert!(
            !workflow_permissions.contains_key(Value::String("packages".to_owned())),
            "workflow must not grant package publish permission at the workflow level",
        );

        let publish_permissions = self
            .job_permissions("publish-image")
            .expect("publish-image job should define explicit permissions");
        assert_eq!(
            publish_permissions
                .get(Value::String("contents".to_owned()))
                .map(value_as_str),
            Some("read"),
            "publish-image job should retain read access for checkout",
        );
        assert_eq!(
            publish_permissions
                .get(Value::String("packages".to_owned()))
                .map(value_as_str),
            None,
            "publish-image job must not request GitHub package publish permission while pushing only to Quay",
        );

        for job_name in [FAST_VALIDATION_JOB_NAME, LONG_VALIDATION_JOB_NAME] {
            assert!(
                self.job_permissions(job_name).is_none(),
                "job `{job_name}` must not elevate permissions",
            );
        }

        let manifest_permissions = self.job_permissions("publish-manifest");
        let manifest_permissions =
            manifest_permissions.expect("publish-manifest job should define explicit permissions");
        assert_eq!(
            manifest_permissions
                .get(Value::String("contents".to_owned()))
                .map(value_as_str),
            Some("read"),
            "publish-manifest job should retain read access for artifact and manifest handling",
        );
        assert_eq!(
            manifest_permissions
                .get(Value::String("packages".to_owned()))
                .map(value_as_str),
            Some("write"),
            "publish-manifest job should have package publish permission to fan out the scanned Quay images into GHCR",
        );

        for job_name in [
            FAST_VALIDATION_JOB_NAME,
            LONG_VALIDATION_JOB_NAME,
            "publish-image",
        ] {
            let checkout_step = self.step_using_prefix(self.job(job_name), "actions/checkout");
            let checkout_inputs = self.step_inputs(checkout_step, "actions/checkout");
            assert_eq!(
                checkout_inputs
                    .get(Value::String("persist-credentials".to_owned()))
                    .map(value_as_bool),
                Some(false),
                "job `{job_name}` checkout must disable credential persistence",
            );
        }

        for job_name in [FAST_VALIDATION_JOB_NAME, LONG_VALIDATION_JOB_NAME] {
            let validation_scripts = self.job_run_scripts(job_name).join("\n");
            assert!(
                !validation_scripts.contains("GITHUB_TOKEN"),
                "job `{job_name}` must not touch publish credentials",
            );
            assert!(
                !validation_scripts.contains("QUAY_ROBOT_USERNAME")
                    && !validation_scripts.contains("QUAY_ROBOT_PASSWORD"),
                "job `{job_name}` must not touch Quay publish credentials",
            );
            assert!(
                !validation_scripts.contains("docker login"),
                "job `{job_name}` must not log in to the registry",
            );
        }
    }

    pub fn assert_publish_is_explicitly_gated_to_the_trusted_main_push_commit(&self) {
        let publish_job = self.job("publish-image");
        let publish_condition = publish_job
            .get(Value::String("if".to_owned()))
            .map(value_as_str)
            .expect("publish-image job should define an explicit trust gate");
        assert!(
            publish_condition.contains("github.event_name == 'push'"),
            "publish-image trust gate must require the push event",
        );
        assert!(
            publish_condition.contains("github.ref == 'refs/heads/main'"),
            "publish-image trust gate must require the main branch ref",
        );
        assert_eq!(
            self.job_needs_names("publish-image"),
            vec![FAST_VALIDATION_JOB_NAME, LONG_VALIDATION_JOB_NAME],
            "publish-image job must wait for both validation boundaries to pass",
        );

        let manifest_job = self.job("publish-manifest");
        let manifest_condition = manifest_job
            .get(Value::String("if".to_owned()))
            .map(value_as_str)
            .expect("publish-manifest job should define an explicit trust gate");
        assert!(
            manifest_condition.contains("github.event_name == 'push'"),
            "publish-manifest trust gate must require the push event",
        );
        assert!(
            manifest_condition.contains("github.ref == 'refs/heads/main'"),
            "publish-manifest trust gate must require the main branch ref",
        );
        assert_eq!(
            self.job_needs_names("publish-manifest"),
            vec![QUAY_SECURITY_GATE_JOB_NAME],
            "publish-manifest job must wait for the Quay security gate before GHCR fan-out",
        );

        let checkout_inputs = self.step_inputs(
            self.step_using_prefix(publish_job, "actions/checkout"),
            "actions/checkout",
        );
        assert!(
            !checkout_inputs.contains_key(Value::String("ref".to_owned())),
            "publish-image checkout must not override the trusted pushed commit ref",
        );

        let security_gate = self.job(QUAY_SECURITY_GATE_JOB_NAME);
        let gate_condition = security_gate
            .get(Value::String("if".to_owned()))
            .map(value_as_str)
            .expect("quay-security-gate job should define an explicit trust gate");
        assert!(
            gate_condition.contains("github.event_name == 'push'"),
            "quay-security-gate trust gate must require the push event",
        );
        assert!(
            gate_condition.contains("github.ref == 'refs/heads/main'"),
            "quay-security-gate trust gate must require the main branch ref",
        );
        assert_eq!(
            self.job_needs_names(QUAY_SECURITY_GATE_JOB_NAME),
            vec!["publish-image"],
            "quay-security-gate job must wait for the Quay image publication lanes",
        );
    }

    pub fn assert_ci_publish_safety_model_is_documented(&self) {
        assert!(
            self.readme_text.contains("## CI Publish Safety"),
            "README should document the CI publish safety model",
        );

        for required_line in [
            "Random pull requests, forks, `pull_request_target`, manual dispatch, reusable workflow calls, scheduled runs, tag pushes, issue-triggered events, and release events do not trigger the protected image-publish workflow.",
            "The `publish-image`, `quay-security-gate`, and `publish-manifest` jobs still carry explicit `if:` gates that require a `push` event on `refs/heads/main`, so widening workflow triggers later does not silently open the protected release path.",
            "Only the `publish-manifest` job gets `packages: write`, checkout disables credential persistence where source is fetched, Quay login uses `--password-stdin`, the Quay scan step uses a temporary netrc file instead of command-line passwords, and every canonical published image still resolves to `${{ github.sha }}` from the validated commit.",
            "Image publication is blocked on explicit `validate-fast` and `validate-long` jobs, so both the default repository validation boundary and the ultra-long lane must pass before any publish step can start.",
            "Both validation jobs restore and save Cargo registry and target caches before publish, each image is first pushed through native `linux/amd64` and `linux/arm64` Quay lanes, the `quay-security-gate` job polls Quay manifest security until every published platform ref is scanned with zero findings, and only then does the manifest job assemble canonical Quay `${{ github.sha }}` tags and fan them out into GHCR while emitting a published-image manifest for downstream consumers.",
        ] {
            assert!(
                self.readme_text.contains(required_line),
                "README should explain CI publish safety invariant: {required_line}",
            );
        }
    }

    pub fn assert_has_explicit_pre_publish_validation_lanes(&self) {
        assert!(
            !self.jobs().contains_key(Value::String("validate".to_owned())),
            "workflow must not keep the old mixed `validate` job around",
        );

        for (job_name, commands) in [
            (FAST_VALIDATION_JOB_NAME, FAST_VALIDATION_COMMANDS.as_slice()),
            (LONG_VALIDATION_JOB_NAME, LONG_VALIDATION_COMMANDS.as_slice()),
        ] {
            let job = self.job(job_name);
            let job_scripts = self
                .steps(job)
                .iter()
                .filter_map(Value::as_mapping)
                .filter_map(|step| step.get(Value::String("run".to_owned())))
                .map(value_as_str)
                .collect::<Vec<_>>();

            for expected_command in commands {
                assert!(
                    job_scripts.iter().any(|script| {
                        script.lines().any(|line| line.trim() == *expected_command)
                    }),
                    "job `{job_name}` must run `{expected_command}` before publishing",
                );
            }
        }

        let publish_needs = self.job_needs_names("publish-image");
        for required_job in [FAST_VALIDATION_JOB_NAME, LONG_VALIDATION_JOB_NAME] {
            assert!(
                publish_needs.contains(&required_job),
                "publish-image job must wait for `{required_job}` before publishing",
            );
        }
    }

    pub fn assert_validation_reuses_host_rust_caches(&self) {
        for (job_name, cache_prefix) in [
            (FAST_VALIDATION_JOB_NAME, "validate-fast"),
            (LONG_VALIDATION_JOB_NAME, "validate-long"),
        ] {
            self.assert_validation_job_reuses_host_rust_caches(job_name, cache_prefix);
        }
    }

    pub fn assert_cancels_older_main_runs_when_new_pushes_arrive(&self) {
        let concurrency = self
            .document
            .as_mapping()
            .expect("workflow document should be a mapping")
            .get(Value::String("concurrency".to_owned()))
            .and_then(Value::as_mapping)
            .expect("workflow should define top-level concurrency");
        assert_eq!(
            concurrency
                .get(Value::String("group".to_owned()))
                .map(value_as_str),
            Some("publish-images-${{ github.ref }}"),
            "workflow concurrency group should track the pushed ref",
        );
        assert_eq!(
            concurrency
                .get(Value::String("cancel-in-progress".to_owned()))
                .map(value_as_bool),
            Some(true),
            "workflow should cancel the previous in-progress run for newer pushes",
        );
    }

    pub fn assert_publishes_the_canonical_three_image_set(&self) {
        let env = self.workflow_env();
        assert_eq!(
            env.get(Value::String("GHCR_REGISTRY".to_owned()))
                .map(value_as_str),
            Some(GHCR_REGISTRY_HOST),
            "workflow should define GHCR once through a shared GHCR_REGISTRY env boundary",
        );
        assert_eq!(
            self.workflow_text.matches("ghcr.io").count(),
            1,
            "workflow should not scatter raw GHCR coordinates across multiple steps",
        );
        assert_eq!(
            env.get(Value::String("QUAY_REGISTRY".to_owned()))
                .map(value_as_str),
            Some(QUAY_REGISTRY_HOST),
            "workflow should define Quay once through a shared QUAY_REGISTRY env boundary",
        );
        assert_eq!(
            env.get(Value::String("QUAY_NAMESPACE".to_owned()))
                .map(value_as_str),
            Some(QUAY_NAMESPACE),
            "workflow should define the hosted-verified Quay namespace through an explicit non-secret boundary",
        );
        assert_eq!(
            self.workflow_text.matches("quay.io").count(),
            1,
            "workflow should not scatter raw Quay coordinates across multiple steps",
        );

        for image in PublishedImageContract::all() {
            let target = ImageBuildTargetContract::find(image.image_id());
            assert_eq!(
                env.get(Value::String(target.repository_env().to_owned()))
                    .map(value_as_str),
                Some(image.repository()),
                "workflow should define `{}` through the shared image contract",
                target.repository_env(),
            );
        }

        for target in ImageBuildTargetContract::all() {
            let matrix_entry = self.publish_matrix_entry(target.image_id());
            assert_eq!(
                matrix_entry
                    .get(Value::String("repository_env".to_owned()))
                    .map(value_as_str),
                Some(target.repository_env()),
                "publish-image matrix should source `{}` from shared build-target metadata",
                target.image_id(),
            );
            assert_eq!(
                matrix_entry
                    .get(Value::String("dockerfile".to_owned()))
                    .map(value_as_str),
                Some(target.dockerfile()),
                "publish-image matrix should preserve the canonical Dockerfile for `{}`",
                target.image_id(),
            );
            assert_eq!(
                matrix_entry
                    .get(Value::String("context".to_owned()))
                    .map(value_as_str),
                Some(target.context()),
                "publish-image matrix should preserve the canonical context for `{}`",
                target.image_id(),
            );
            assert_eq!(
                matrix_entry
                    .get(Value::String("manifest_key".to_owned()))
                    .map(value_as_str),
                Some(target.manifest_key()),
                "publish-image matrix should preserve the canonical manifest key for `{}`",
                target.image_id(),
            );
            assert_eq!(
                matrix_entry
                    .get(Value::String("artifact_name".to_owned()))
                    .map(value_as_str),
                Some(target.artifact_name()),
                "publish-image matrix should preserve the canonical artifact name for `{}`",
                target.image_id(),
            );
            assert_eq!(
                matrix_entry
                    .get(Value::String("cache_scope".to_owned()))
                    .map(value_as_str),
                Some(target.cache_scope()),
                "publish-image matrix should preserve the canonical cache scope for `{}`",
                target.image_id(),
            );
            assert_eq!(
                matrix_entry
                    .get(Value::String("build_kind".to_owned()))
                    .map(value_as_str),
                Some(target.build_kind()),
                "publish-image matrix should preserve the canonical build kind for `{}`",
                target.image_id(),
            );
        }
    }

    pub fn assert_uses_multi_arch_commit_sha_tags_only(&self) {
        let publish_script = self.step_run_script(
            self.step_named(self.job("publish-image"), "Publish image"),
            "Publish image",
        );
        assert!(
            publish_script.contains("docker buildx build"),
            "publish-image job must use `docker buildx build`",
        );
        assert!(
            publish_script.contains("--platform \"${{ matrix.platform.platform }}\""),
            "publish-image job must publish one native platform per lane",
        );
        assert!(
            !publish_script.contains("--platform linux/amd64,linux/arm64"),
            "publish-image job must stop using the old combined multi-arch build invocation",
        );
        assert!(
            publish_script.contains("${{ env.QUAY_REGISTRY }}/"),
            "publish-image job must tag Quay through the shared registry boundary",
        );
        assert!(
            publish_script.contains("${{ env.QUAY_NAMESPACE }}/${image_repository}"),
            "publish-image job must build Quay repository coordinates from the shared namespace and image repository metadata",
        );
        assert!(
            publish_script.contains("${{ github.sha }}"),
            "publish-image job must tag only the pushed commit SHA",
        );
        assert!(
            publish_script.contains("--tag \"${platform_image_ref}\""),
            "publish-image job must push a platform-specific ref for manifest fan-in",
        );
        assert!(
            publish_script.contains("--push"),
            "publish-image job must push after a successful build",
        );

        let manifest_script = self.step_run_script(
            self.step_named(self.job("publish-manifest"), "Publish manifest"),
            "Publish manifest",
        );
        for required_marker in [
            "docker buildx imagetools create",
            "--tag \"${final_image_ref}\"",
            "${amd64_ref}",
            "${arm64_ref}",
            "docker buildx imagetools inspect",
            "skopeo copy",
            "--all",
            "docker://${final_image_ref}",
            "docker://${ghcr_final_image_ref}",
        ] {
            assert!(
                manifest_script.contains(required_marker),
                "publish-manifest job must preserve the Quay-manifest then GHCR-fan-out path through `{required_marker}`",
            );
        }

        for forbidden_marker in ["latest", "refs/tags/", "github.ref_name", "type=semver"] {
            assert!(
                !publish_script.contains(forbidden_marker) && !manifest_script.contains(forbidden_marker),
                "publish workflow must not introduce `{forbidden_marker}` tags",
            );
        }
    }

    pub fn assert_installs_publish_dependencies_via_direct_shell_steps(&self) {
        for job_name in [FAST_VALIDATION_JOB_NAME, LONG_VALIDATION_JOB_NAME] {
            let validate_install = self.step_run_script(
                self.step_named(self.job(job_name), "Install Rust toolchain"),
                "Install Rust toolchain",
            );
            assert!(
                validate_install.contains("rustup toolchain install 1.93.0"),
                "job `{job_name}` should install the pinned Rust toolchain directly via shell",
            );
            assert!(
                validate_install.contains("rustup default 1.93.0"),
                "job `{job_name}` should activate the pinned Rust toolchain directly via shell",
            );
            assert!(
                validate_install.contains("sudo apt-get install --yes postgresql"),
                "job `{job_name}` should install PostgreSQL tooling needed by the contract tests",
            );
            assert!(
                validate_install.contains("find /usr/lib/postgresql"),
                "job `{job_name}` should discover the installed PostgreSQL bin directory explicitly",
            );
            assert!(
                validate_install.contains("GITHUB_PATH"),
                "job `{job_name}` should add PostgreSQL binaries to PATH for later test steps",
            );
        }

        let publish_install = self.step_run_script(
            self.step_named(self.job("publish-image"), "Install publish dependencies"),
            "Install publish dependencies",
        );
        for required_marker in [
            "sudo apt-get update",
            "case \"${{ runner.arch }}\" in",
            "buildx_arch=amd64",
            "buildx_arch=arm64",
            "curl -fsSL",
            "docker buildx version",
            "docker buildx create",
            "docker buildx inspect --bootstrap",
        ] {
            assert!(
                publish_install.contains(required_marker),
                "publish dependency installation must include `{required_marker}`",
            );
        }
        for forbidden_marker in ["qemu-user-static", "update-binfmts", "binfmt-support"] {
            assert!(
                !publish_install.contains(forbidden_marker),
                "publish dependency installation must not keep the old emulated arm64 dependency `{forbidden_marker}`",
            );
        }

        let manifest_install = self.step_run_script(
            self.step_named(self.job("publish-manifest"), "Install publish dependencies"),
            "Install publish dependencies",
        );
        for required_marker in [
            "sudo apt-get install --yes skopeo",
            "BUILDX_VERSION=v0.30.1",
            "case \"${{ runner.arch }}\" in",
            "curl -fsSL",
            "docker buildx version",
        ] {
            assert!(
                manifest_install.contains(required_marker),
                "publish-manifest dependency installation must include `{required_marker}`",
            );
        }

        let scan_install = self.step_run_script(
            self.step_named(self.job(QUAY_SECURITY_GATE_JOB_NAME), "Install Quay scan dependencies"),
            "Install Quay scan dependencies",
        );
        assert!(
            scan_install.contains("sudo apt-get install --yes jq"),
            "quay-security-gate dependency installation must include jq for scan result parsing",
        );

        for forbidden_action in [
            "dtolnay/rust-toolchain",
            "docker/setup-buildx-action",
            "docker/login-action",
            "docker/build-push-action",
            "aquasecurity/setup-trivy",
        ] {
            assert!(
                !self.workflow_text.contains(forbidden_action),
                "workflow must not depend on `{forbidden_action}`",
            );
        }
    }

    pub fn assert_proves_native_runner_matches_each_platform_lane_before_manifesting(&self) {
        let publish_job = self.job("publish-image");
        let probe_script = self.step_run_script(
            self.step_named(publish_job, "Assert native platform runner"),
            "Assert native platform runner",
        );
        for required_marker in [
            "runner.arch",
            "publish platform",
            "expected_platform",
            "linux/amd64",
            "linux/arm64",
            "test \"${expected_platform}\" = \"${{ matrix.platform.platform }}\"",
        ] {
            assert!(
                probe_script.contains(required_marker),
                "native publish-lane proof step must include `{required_marker}`",
            );
        }

        let publish_script =
            self.step_run_script(self.step_named(publish_job, "Publish image"), "Publish image");
        assert!(
            publish_script.contains("--progress plain"),
            "publish-image job must use plain buildx progress so hosted failures stay inspectable",
        );

        let manifest_script = self.step_run_script(
            self.step_named(self.job("publish-manifest"), "Publish manifest"),
            "Publish manifest",
        );
        assert!(
            manifest_script.contains("docker buildx imagetools create"),
            "publish-manifest job must assemble the final multi-arch Quay tag from the native platform pushes",
        );
    }

    pub fn assert_emits_published_image_manifest_for_downstream_consumers(&self) {
        let manifest_job = self.job("publish-manifest");
        let manifest_env = self
            .job_env("publish-manifest")
            .expect("publish-manifest job should define an env mapping");
        let publish_env = self
            .job_env("publish-image")
            .expect("publish-image job should define an env mapping");
        let outputs = manifest_job
            .get(Value::String("outputs".to_owned()))
            .and_then(Value::as_mapping)
            .expect("publish-manifest job should expose outputs for downstream consumers");
        assert_eq!(
            outputs
                .get(Value::String("publish_manifest".to_owned()))
                .map(value_as_str),
            Some("${{ steps.publish-manifest.outputs.publish_manifest }}"),
            "publish-manifest job should expose the manifest output",
        );

        let manifest_step = self.step_named(manifest_job, "Publish manifest");
        let manifest_script = self.step_run_script(manifest_step, "Publish manifest");
        assert_eq!(
            publish_env
                .get(Value::String("PUBLISHED_IMAGE_REF_FILE".to_owned()))
                .map(value_as_str),
            Some(
                "${{ github.workspace }}/published-image-refs/${{ matrix.image.image_id }}-${{ matrix.platform.platform_tag_suffix }}-image-ref.env"
            ),
            "publish-image job should keep the per-lane image-ref artifact on a workspace-scoped path that GitHub accepts at job-env evaluation time",
        );
        assert_eq!(
            manifest_env
                .get(Value::String("PUBLISHED_IMAGE_MANIFEST".to_owned()))
                .map(value_as_str),
            Some("${{ github.workspace }}/published-images.json"),
            "publish-manifest job should scope the manifest path to the checked-out workspace",
        );
        assert!(
            manifest_script.contains("${{ env.PUBLISHED_IMAGE_MANIFEST }}"),
            "manifest step should write the published image manifest through the publish-manifest env boundary",
        );
        assert!(
            manifest_script.contains("GITHUB_OUTPUT"),
            "manifest step should expose published image refs through job outputs",
        );

        let upload_inputs = self.step_inputs(
            self.step_using_prefix(manifest_job, "actions/upload-artifact"),
            "actions/upload-artifact",
        );
        assert_eq!(
            upload_inputs
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("${{ env.PUBLISHED_IMAGE_MANIFEST }}"),
            "manifest artifact upload should preserve the shared manifest path boundary",
        );
        assert_eq!(
            upload_inputs
                .get(Value::String("if-no-files-found".to_owned()))
                .map(value_as_str),
            Some("error"),
            "manifest artifact upload must fail loudly if the manifest is missing",
        );

        let download_inputs = self.step_inputs(
            self.step_using_prefix(manifest_job, "actions/download-artifact"),
            "actions/download-artifact",
        );
        assert_eq!(
            download_inputs
                .get(Value::String("pattern".to_owned()))
                .map(value_as_str),
            Some("published-image-*"),
            "manifest job should download the per-image publication artifacts through a shared pattern",
        );
        assert_eq!(
            download_inputs
                .get(Value::String("merge-multiple".to_owned()))
                .map(value_as_bool),
            Some(true),
            "manifest job should merge the per-image artifacts into one directory",
        );

        let publish_upload_inputs = self.step_inputs(
            self.step_named(self.job("publish-image"), "Upload published image ref artifact"),
            "Upload published image ref artifact",
        );
        assert_eq!(
            publish_upload_inputs
                .get(Value::String("name".to_owned()))
                .map(value_as_str),
            Some("${{ matrix.image.artifact_name }}-${{ matrix.platform.platform_tag_suffix }}"),
            "publish-image job should upload one artifact per image/platform lane through the shared matrix metadata",
        );
        assert_eq!(
            publish_upload_inputs
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("${{ env.PUBLISHED_IMAGE_REF_FILE }}"),
            "publish-image job should upload the shared image-ref file path",
        );

        for target in ImageBuildTargetContract::all() {
            let expected_output =
                format!("${{{{ steps.publish-manifest.outputs.{} }}}}", target.manifest_key());
            assert_eq!(
                outputs
                    .get(Value::String(target.manifest_key().to_owned()))
                    .map(value_as_str),
                Some(expected_output.as_str()),
                "publish-manifest job should expose `{}` through the manifest step",
                target.manifest_key(),
            );
        }
        for required_marker in ["manifest_keys", "output_lines", "publish_manifest<<EOF"] {
            assert!(
                manifest_script.contains(required_marker),
                "manifest step should keep `{required_marker}` within the shared manifest/output boundary",
            );
        }
    }

    pub fn assert_masks_derived_sensitive_values_and_never_logs_raw_credentials(&self) {
        let mask_script = self.step_run_script(
            self.step_named(self.job("publish-image"), "Mask derived Quay publish credentials"),
            "Mask derived Quay publish credentials",
        );
        assert!(
            mask_script.contains("quay_basic_auth"),
            "workflow should derive a Quay composite credential for masking verification",
        );
        assert!(
            mask_script.contains("::add-mask::${quay_basic_auth}"),
            "workflow should explicitly mask the derived Quay credential before any output",
        );
        assert!(
            mask_script.contains("Quay publish auth (masked)"),
            "workflow should print a masked Quay diagnostic so hosted logs can prove redaction works",
        );

        let login_script = self.step_run_script(
            self.step_named(self.job("publish-image"), "Login to Quay"),
            "Login to Quay",
        );
        assert!(
            login_script.contains("docker login"),
            "publish-image job should log in to Quay explicitly through shell",
        );
        assert!(
            login_script.contains("--password-stdin"),
            "publish-image job must pass credentials through stdin instead of echoing them into the command line",
        );
        assert!(
            !login_script.contains("--password "),
            "publish-image job must not pass credentials as a shell argument",
        );

        let ghcr_mask_script = self.step_run_script(
            self.step_named(self.job("publish-manifest"), "Mask derived GHCR publish credentials"),
            "Mask derived GHCR publish credentials",
        );
        assert!(
            ghcr_mask_script.contains("derived_registry_auth"),
            "publish-manifest job should derive a GHCR composite credential for masking verification",
        );
        assert!(
            ghcr_mask_script.contains("::add-mask::${derived_registry_auth}"),
            "publish-manifest job should explicitly mask the GHCR credential before any output",
        );
        assert!(
            ghcr_mask_script.contains("GHCR publish auth (masked)"),
            "publish-manifest job should print a masked GHCR diagnostic so hosted logs can prove redaction works",
        );
    }

    pub fn assert_parallel_publish_topology_uses_shared_build_targets(&self) {
        assert!(
            !self
                .document
                .as_mapping()
                .expect("workflow document should be a mapping")
                .get(Value::String("jobs".to_owned()))
                .and_then(Value::as_mapping)
                .expect("workflow should define jobs")
                .contains_key(Value::String("publish".to_owned())),
            "workflow should remove the old sequential `publish` bottleneck job",
        );

        let publish_job = self.job("publish-image");
        assert_eq!(
            publish_job
                .get(Value::String("runs-on".to_owned()))
                .map(value_as_str),
            Some("${{ matrix.platform.runner }}"),
            "publish-image job should route each platform lane through shared matrix runner metadata",
        );
        let strategy = publish_job
            .get(Value::String("strategy".to_owned()))
            .and_then(Value::as_mapping)
            .expect("publish-image job should declare a strategy");
        assert_eq!(
            strategy
                .get(Value::String("fail-fast".to_owned()))
                .map(value_as_bool),
            Some(false),
            "publish-image matrix should not cancel unrelated image targets on the first failure",
        );
        assert_eq!(
            self.publish_matrix_entries().len(),
            ImageBuildTargetContract::all().len(),
            "publish-image image axis should cover the canonical image target set exactly once",
        );
        assert_eq!(
            self.publish_platform_matrix_entries().len(),
            2,
            "publish-image platform axis should keep amd64 and arm64 publication independent",
        );
        assert_eq!(
            self.job_needs_names("publish-manifest"),
            vec![QUAY_SECURITY_GATE_JOB_NAME],
            "publish-manifest should aggregate only after the Quay security gate completes",
        );
    }

    pub fn assert_uploads_the_source_controlled_compose_artifacts(&self) {
        let publish_manifest_job = self.job("publish-manifest");
        let checkout_inputs = self.step_inputs(
            self.step_using_prefix(publish_manifest_job, "actions/checkout"),
            "actions/checkout",
        );
        assert_eq!(
            checkout_inputs
                .get(Value::String("persist-credentials".to_owned()))
                .map(value_as_bool),
            Some(false),
            "publish-manifest checkout must disable credential persistence when uploading source-controlled compose artifacts",
        );

        let upload_inputs = self.step_inputs(
            self.step_named(publish_manifest_job, "Upload published compose artifacts"),
            "Upload published compose artifacts",
        );
        assert_eq!(
            upload_inputs
                .get(Value::String("name".to_owned()))
                .map(value_as_str),
            Some("published-compose-artifacts"),
            "publish workflow must upload the checked-in compose files through one stable artifact boundary",
        );
        assert_eq!(
            upload_inputs
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("artifacts/compose"),
            "publish workflow must upload the source-controlled compose artifact directory directly",
        );
        assert_eq!(
            upload_inputs
                .get(Value::String("if-no-files-found".to_owned()))
                .map(value_as_str),
            Some("error"),
            "compose artifact upload must fail loudly if the checked-in compose files disappear",
        );
    }

    pub fn assert_publish_jobs_use_remote_buildkit_caches(&self) {
        let publish_script = self.step_run_script(
            self.step_named(self.job("publish-image"), "Publish image"),
            "Publish image",
        );
        assert!(
            publish_script.contains(
                "--cache-from \"type=gha,scope=${{ matrix.image.cache_scope }}-${{ matrix.platform.platform_tag_suffix }}\""
            ),
            "publish-image job must restore BuildKit cache state from the image/platform cache scope",
        );
        assert!(
            publish_script.contains(
                "--cache-to \"type=gha,scope=${{ matrix.image.cache_scope }}-${{ matrix.platform.platform_tag_suffix }},mode=max\""
            ),
            "publish-image job must save BuildKit cache state back to the image/platform cache scope",
        );

        for target in ImageBuildTargetContract::all() {
            assert!(
                !target.cache_scope().is_empty(),
                "shared build target `{}` must define a non-empty cache scope",
                target.image_id(),
            );
        }
    }

    pub fn assert_uses_native_arm64_publish_lanes(&self) {
        let publish_job = self.job("publish-image");
        assert_eq!(
            publish_job
                .get(Value::String("runs-on".to_owned()))
                .map(value_as_str),
            Some("${{ matrix.platform.runner }}"),
            "publish-image job should route each platform lane onto its own runner boundary",
        );

        let publish_script =
            self.step_run_script(self.step_named(publish_job, "Publish image"), "Publish image");
        assert!(
            !publish_script.contains("--platform linux/amd64,linux/arm64"),
            "publish-image job must stop publishing both architectures from one combined build invocation",
        );

        let platform_matrix = self.publish_platform_matrix_entries();
        assert_eq!(
            platform_matrix.len(),
            2,
            "publish-image job should define exactly two platform lanes",
        );

        let arm64_lane = self.publish_platform_matrix_entry("linux/arm64");
        assert_eq!(
            arm64_lane
                .get(Value::String("runner".to_owned()))
                .map(value_as_str),
            Some("ubuntu-24.04-arm"),
            "linux/arm64 publication should run on the native hosted arm64 runner",
        );
    }

    pub fn assert_publishes_to_quay_before_any_ghcr_fan_out(&self) {
        let publish_script =
            self.step_run_script(self.step_named(self.job("publish-image"), "Publish image"), "Publish image");
        assert!(
            publish_script.contains("${{ env.QUAY_REGISTRY }}/"),
            "publish-image job must publish into Quay first",
        );
        assert!(
            !publish_script.contains("${{ env.GHCR_REGISTRY }}/"),
            "publish-image job must not publish directly into GHCR",
        );

        let manifest_script = self.step_run_script(
            self.step_named(self.job("publish-manifest"), "Publish manifest"),
            "Publish manifest",
        );
        assert_eq!(
            self.job_needs_names("publish-manifest"),
            vec![QUAY_SECURITY_GATE_JOB_NAME],
            "publish-manifest job must wait for the Quay security gate before GHCR fan-out",
        );
        assert!(
            manifest_script.contains("ghcr_final_image_ref"),
            "publish-manifest job must derive downstream GHCR refs only after the Quay path is available",
        );
        assert!(
            manifest_script.contains("docker://${final_image_ref}")
                && manifest_script.contains("docker://${ghcr_final_image_ref}"),
            "publish-manifest job must copy from Quay refs into GHCR refs",
        );
        assert_script_order(
            manifest_script,
            "docker buildx imagetools create",
            "skopeo copy",
            "publish-manifest job must create the canonical Quay manifest before the GHCR copy begins",
        );
    }

    pub fn assert_scopes_quay_credentials_to_publish_and_scan_only(&self) {
        for job_name in [FAST_VALIDATION_JOB_NAME, LONG_VALIDATION_JOB_NAME] {
            let scripts = self.job_run_scripts(job_name).join("\n");
            for forbidden_marker in [
                "QUAY_ROBOT_USERNAME",
                "QUAY_ROBOT_PASSWORD",
                "--netrc-file",
                "/api/v1/repository/",
                "docker login \"${{ env.QUAY_REGISTRY }}\"",
            ] {
                assert!(
                    !scripts.contains(forbidden_marker),
                    "job `{job_name}` must keep Quay credentials and API access out of validation through `{forbidden_marker}`",
                );
            }
        }

        let gate_script = self.step_run_script(
            self.step_named(self.job(QUAY_SECURITY_GATE_JOB_NAME), "Wait for Quay security gate"),
            "Wait for Quay security gate",
        );
        for required_marker in [
            "--netrc-file",
            "cat > \"${netrc_file}\" <<EOF",
            "machine ${{ env.QUAY_REGISTRY }}",
            "login ${QUAY_ROBOT_USERNAME}",
            "password ${QUAY_ROBOT_PASSWORD}",
        ] {
            assert!(
                gate_script.contains(required_marker),
                "quay-security-gate job must scope API credentials through `{required_marker}`",
            );
        }
        for forbidden_marker in [
            "curl -u",
            "Authorization: Basic",
            "Authorization: Bearer",
            "--password ",
        ] {
            assert!(
                !gate_script.contains(forbidden_marker),
                "quay-security-gate job must not leak Quay credentials through `{forbidden_marker}`",
            );
        }
    }

    pub fn assert_requires_a_quay_security_gate_before_publication_finishes(&self) {
        assert_eq!(
            self.job_needs_names(QUAY_SECURITY_GATE_JOB_NAME),
            vec!["publish-image"],
            "quay-security-gate job must start only after the Quay publish lanes complete",
        );
        assert_eq!(
            self.job_needs_names("publish-manifest"),
            vec![QUAY_SECURITY_GATE_JOB_NAME],
            "publish-manifest job must stay blocked on the Quay security gate",
        );

        let gate_script = self.step_run_script(
            self.step_named(self.job(QUAY_SECURITY_GATE_JOB_NAME), "Wait for Quay security gate"),
            "Wait for Quay security gate",
        );
        for required_marker in [
            "/api/v1/repository/${quay_repository}/tag/?specificTag=${tag_name}",
            "/api/v1/repository/${quay_repository}/manifest/${manifest_digest}/security?vulnerabilities=true",
            "scan_status",
            "vulnerability_count",
            "Quay vulnerability gate failed",
            "Quay scan passed",
            "test \"${scan_status}\" = \"scanned\"",
        ] {
            assert!(
                gate_script.contains(required_marker),
                "quay-security-gate job must enforce the scan boundary through `{required_marker}`",
            );
        }
        assert!(
            gate_script.contains("exit 1"),
            "quay-security-gate job must fail loudly when Quay reports findings or an indeterminate scan state",
        );
    }

    fn workflow_on(&self) -> &Mapping {
        self.document
            .as_mapping()
            .expect("workflow document should be a mapping")
            .get(Value::String("on".to_owned()))
            .or_else(|| {
                self.document
                    .as_mapping()
                    .expect("workflow document should be a mapping")
                    .get(Value::Bool(true))
            })
            .and_then(Value::as_mapping)
            .expect("workflow should define an `on` mapping")
    }

    fn workflow_permissions(&self) -> Option<&Mapping> {
        self.document
            .as_mapping()
            .expect("workflow document should be a mapping")
            .get(Value::String("permissions".to_owned()))
            .and_then(Value::as_mapping)
    }

    fn workflow_env(&self) -> &Mapping {
        self.document
            .as_mapping()
            .expect("workflow document should be a mapping")
            .get(Value::String("env".to_owned()))
            .and_then(Value::as_mapping)
            .expect("workflow should define a shared env mapping")
    }

    fn push_branches(&self) -> Vec<&str> {
        self.push_mapping()
            .get(Value::String("branches".to_owned()))
            .and_then(Value::as_sequence)
            .expect("workflow `on.push.branches` should be a sequence")
            .iter()
            .map(value_as_str)
            .collect()
    }

    fn push_tags(&self) -> Option<Vec<&str>> {
        self.push_mapping()
            .get(Value::String("tags".to_owned()))
            .map(|tags| {
                tags.as_sequence()
                    .expect("workflow `on.push.tags` should be a sequence")
                    .iter()
                    .map(value_as_str)
                    .collect()
            })
    }

    fn push_mapping(&self) -> &Mapping {
        self.workflow_on()
            .get(Value::String("push".to_owned()))
            .and_then(Value::as_mapping)
            .expect("workflow `on.push` should be a mapping")
    }

    fn jobs(&self) -> &Mapping {
        self.document
            .as_mapping()
            .expect("workflow document should be a mapping")
            .get(Value::String("jobs".to_owned()))
            .and_then(Value::as_mapping)
            .expect("workflow should define a jobs mapping")
    }

    fn job(&self, name: &str) -> &Mapping {
        self.jobs()
            .get(Value::String(name.to_owned()))
            .and_then(Value::as_mapping)
            .unwrap_or_else(|| panic!("workflow should define a `{name}` job"))
    }

    fn job_env(&self, name: &str) -> Option<&Mapping> {
        self.job(name)
            .get(Value::String("env".to_owned()))
            .and_then(Value::as_mapping)
    }

    fn job_permissions(&self, name: &str) -> Option<&Mapping> {
        self.job(name)
            .get(Value::String("permissions".to_owned()))
            .and_then(Value::as_mapping)
    }

    fn job_needs_names(&self, name: &str) -> Vec<&str> {
        match self.job(name).get(Value::String("needs".to_owned())) {
            Some(Value::String(single_need)) => vec![single_need.as_str()],
            Some(Value::Sequence(needs)) => needs.iter().map(value_as_str).collect(),
            Some(other) => panic!("workflow `needs` should be string or sequence: {other:?}"),
            None => Vec::new(),
        }
    }

    fn job_run_scripts(&self, name: &str) -> Vec<&str> {
        self.steps(self.job(name))
            .iter()
            .filter_map(Value::as_mapping)
            .filter_map(|step| step.get(Value::String("run".to_owned())))
            .map(value_as_str)
            .collect()
    }

    fn assert_validation_job_reuses_host_rust_caches(&self, job_name: &str, cache_prefix: &str) {
        let restore_registry = self.step_inputs(
            self.step_named(self.job(job_name), "Restore Cargo registry cache"),
            "Restore Cargo registry cache",
        );
        let restore_target = self.step_inputs(
            self.step_named(self.job(job_name), "Restore Cargo target cache"),
            "Restore Cargo target cache",
        );
        let save_registry = self.step_inputs(
            self.step_named(self.job(job_name), "Save Cargo registry cache"),
            "Save Cargo registry cache",
        );
        let save_target = self.step_inputs(
            self.step_named(self.job(job_name), "Save Cargo target cache"),
            "Save Cargo target cache",
        );

        for (step_name, inputs, expected_uses) in [
            (
                "Restore Cargo registry cache",
                restore_registry,
                "actions/cache/restore",
            ),
            (
                "Restore Cargo target cache",
                restore_target,
                "actions/cache/restore",
            ),
            ("Save Cargo registry cache", save_registry, "actions/cache/save"),
            ("Save Cargo target cache", save_target, "actions/cache/save"),
        ] {
            let step = self.step_named(self.job(job_name), step_name);
            let uses = step
                .get(Value::String("uses".to_owned()))
                .map(value_as_str)
                .expect("cache step should declare a cache action");
            assert!(
                uses.starts_with(expected_uses),
                "`{step_name}` in `{job_name}` should use `{expected_uses}`",
            );
            assert!(
                inputs.contains_key(Value::String("key".to_owned())),
                "`{step_name}` in `{job_name}` should declare an explicit cache key",
            );
        }

        assert_eq!(
            restore_registry
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("~/.cargo/registry/cache\n~/.cargo/registry/index\n~/.cargo/git/db\n"),
            "job `{job_name}` should restore the Cargo registry and git dependency caches",
        );
        assert_eq!(
            restore_target
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("target"),
            "job `{job_name}` should restore the shared workspace target cache",
        );
        assert_eq!(
            save_registry
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("~/.cargo/registry/cache\n~/.cargo/registry/index\n~/.cargo/git/db\n"),
            "job `{job_name}` should save the Cargo registry and git dependency caches",
        );
        assert_eq!(
            save_target
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("target"),
            "job `{job_name}` should save the shared workspace target cache",
        );
        assert!(
            restore_registry
                .get(Value::String("key".to_owned()))
                .map(value_as_str)
                .is_some_and(|key| key.contains(&format!("{cache_prefix}-cargo-registry"))),
            "job `{job_name}` registry cache key should stay explicit and stable",
        );
        assert!(
            restore_target
                .get(Value::String("key".to_owned()))
                .map(value_as_str)
                .is_some_and(|key| key.contains(&format!("{cache_prefix}-target"))),
            "job `{job_name}` target cache key should stay explicit and stable",
        );
    }

    fn publish_matrix_entries(&self) -> &[Value] {
        self.job("publish-image")
            .get(Value::String("strategy".to_owned()))
            .and_then(Value::as_mapping)
            .and_then(|strategy| strategy.get(Value::String("matrix".to_owned())))
            .and_then(Value::as_mapping)
            .and_then(|matrix| matrix.get(Value::String("image".to_owned())))
            .and_then(Value::as_sequence)
            .map(Vec::as_slice)
            .expect("publish-image job should define a matrix.image sequence")
    }

    fn publish_platform_matrix_entries(&self) -> &[Value] {
        self.job("publish-image")
            .get(Value::String("strategy".to_owned()))
            .and_then(Value::as_mapping)
            .and_then(|strategy| strategy.get(Value::String("matrix".to_owned())))
            .and_then(Value::as_mapping)
            .and_then(|matrix| matrix.get(Value::String("platform".to_owned())))
            .and_then(Value::as_sequence)
            .map(Vec::as_slice)
            .expect("publish-image job should define a matrix.platform sequence")
    }

    fn publish_platform_matrix_entry(&self, platform: &str) -> &Mapping {
        self.publish_platform_matrix_entries()
            .iter()
            .filter_map(Value::as_mapping)
            .find(|entry| {
                entry
                    .get(Value::String("platform".to_owned()))
                    .and_then(Value::as_str)
                    .is_some_and(|candidate| candidate == platform)
            })
            .unwrap_or_else(|| panic!("publish-image matrix should define platform `{platform}`"))
    }

    fn publish_matrix_entry(&self, image_id: &str) -> &Mapping {
        self.publish_matrix_entries()
            .iter()
            .filter_map(Value::as_mapping)
            .find(|entry| {
                entry
                    .get(Value::String("image_id".to_owned()))
                    .and_then(Value::as_str)
                    .is_some_and(|candidate| candidate == image_id)
            })
            .unwrap_or_else(|| panic!("publish-image matrix should define `{image_id}`"))
    }

    fn steps<'a>(&self, job: &'a Mapping) -> &'a [Value] {
        job.get(Value::String("steps".to_owned()))
            .and_then(Value::as_sequence)
            .map(Vec::as_slice)
            .expect("workflow job should define steps")
    }

    fn step_named<'a>(&self, job: &'a Mapping, step_name: &str) -> &'a Mapping {
        self.steps(job)
            .iter()
            .filter_map(Value::as_mapping)
            .find(|step| {
                step.get(Value::String("name".to_owned()))
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == step_name)
            })
            .unwrap_or_else(|| panic!("workflow job should define a `{step_name}` step"))
    }

    fn step_using_prefix<'a>(&self, job: &'a Mapping, uses_prefix: &str) -> &'a Mapping {
        self.steps(job)
            .iter()
            .filter_map(Value::as_mapping)
            .find(|step| {
                step.get(Value::String("uses".to_owned()))
                    .and_then(Value::as_str)
                    .is_some_and(|uses| uses.starts_with(uses_prefix))
            })
            .unwrap_or_else(|| panic!("workflow job should use `{uses_prefix}`"))
    }

    fn step_inputs<'a>(&self, step: &'a Mapping, step_name: &str) -> &'a Mapping {
        step.get(Value::String("with".to_owned()))
            .and_then(Value::as_mapping)
            .unwrap_or_else(|| panic!("workflow step `{step_name}` should define explicit inputs"))
    }

    fn step_run_script<'a>(&self, step: &'a Mapping, step_name: &str) -> &'a str {
        step.get(Value::String("run".to_owned()))
            .map(value_as_str)
            .unwrap_or_else(|| panic!("workflow step `{step_name}` should define a run script"))
    }
}

fn value_as_str(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("workflow value should be a string: {value:?}"))
}

fn value_as_bool(value: &Value) -> bool {
    match value {
        Value::Bool(boolean) => *boolean,
        Value::String(boolean) => boolean == "true",
        _ => panic!("workflow value should be a boolean: {value:?}"),
    }
}

fn assert_script_order(script: &str, first: &str, second: &str, context: &str) {
    let first_index = script
        .find(first)
        .unwrap_or_else(|| panic!("{context}: missing `{first}`"));
    let second_index = script
        .find(second)
        .unwrap_or_else(|| panic!("{context}: missing `{second}`"));
    assert!(
        first_index < second_index,
        "{context}: `{first}` must appear before `{second}`",
    );
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}
