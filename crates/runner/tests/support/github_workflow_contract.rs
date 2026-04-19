use std::{fs, path::PathBuf};

use serde_yaml::{Mapping, Value};

use crate::image_build_target_contract_support::ImageBuildTargetContract;
use crate::published_image_contract_support::PublishedImageContract;

pub struct GithubWorkflowContract {
    workflow_text: String,
    readme_text: String,
    document: Value,
}

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
            Some("write"),
            "publish-image job should be the only job with package publish permission",
        );

        assert!(
            self.job_permissions("validate").is_none(),
            "validate job must not elevate permissions",
        );

        let manifest_permissions = self.job_permissions("publish-manifest");
        assert!(
            manifest_permissions.is_none()
                || !manifest_permissions
                    .expect("publish-manifest permissions should parse")
                    .contains_key(Value::String("packages".to_owned())),
            "publish-manifest job must not gain package publish permissions",
        );

        for job_name in ["validate", "publish-image"] {
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

        let validate_scripts = self.job_run_scripts("validate").join("\n");
        assert!(
            !validate_scripts.contains("GITHUB_TOKEN"),
            "validate job must not touch publish credentials",
        );
        assert!(
            !validate_scripts.contains("docker login"),
            "validate job must not log in to the registry",
        );
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
            vec!["validate"],
            "publish-image job must wait for validation to pass",
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
            vec!["publish-image"],
            "publish-manifest job must wait for the parallel image publication job family",
        );

        let checkout_inputs = self.step_inputs(
            self.step_using_prefix(publish_job, "actions/checkout"),
            "actions/checkout",
        );
        assert!(
            !checkout_inputs.contains_key(Value::String("ref".to_owned())),
            "publish-image checkout must not override the trusted pushed commit ref",
        );
    }

    pub fn assert_ci_publish_safety_model_is_documented(&self) {
        assert!(
            self.readme_text.contains("## CI Publish Safety"),
            "README should document the CI publish safety model",
        );

        for required_line in [
            "Random pull requests, forks, `pull_request_target`, manual dispatch, reusable workflow calls, scheduled runs, tag pushes, issue-triggered events, and release events do not trigger the protected image-publish workflow.",
            "The `publish-image` and `publish-manifest` jobs still carry an explicit `if:` gate that requires a `push` event on `refs/heads/main`, so widening workflow triggers later does not silently open the release path.",
            "Only the `publish-image` job gets `packages: write`, checkout disables credential persistence, derived registry credentials are masked before any diagnostic output, and the pushed images are tagged only with `${{ github.sha }}` from the validated commit.",
            "Validation restores and saves Cargo registry and target caches before publish, each image is pushed for both `linux/amd64` and `linux/arm64`, and the workflow emits a published-image manifest that downstream registry-only checks can consume directly.",
        ] {
            assert!(
                self.readme_text.contains(required_line),
                "README should explain CI publish safety invariant: {required_line}",
            );
        }
    }

    pub fn assert_runs_validation_commands(&self, expected_commands: &[&str]) {
        let validate_job = self.job("validate");
        let validate_scripts = self
            .steps(validate_job)
            .iter()
            .filter_map(Value::as_mapping)
            .filter_map(|step| step.get(Value::String("run".to_owned())))
            .map(value_as_str)
            .collect::<Vec<_>>();

        for expected_command in expected_commands {
            assert!(
                validate_scripts.iter().any(|script| {
                    script.lines().any(|line| line.trim() == *expected_command)
                }),
                "validate job must run `{expected_command}`",
            );
        }

        assert!(
            !validate_scripts.iter().any(|script| script.contains("make test-long")),
            "workflow must not run `make test-long` for this task",
        );
    }

    pub fn assert_validation_reuses_host_rust_caches(&self) {
        let restore_registry = self.step_inputs(
            self.step_named(self.job("validate"), "Restore Cargo registry cache"),
            "Restore Cargo registry cache",
        );
        let restore_target = self.step_inputs(
            self.step_named(self.job("validate"), "Restore Cargo target cache"),
            "Restore Cargo target cache",
        );
        let save_registry = self.step_inputs(
            self.step_named(self.job("validate"), "Save Cargo registry cache"),
            "Save Cargo registry cache",
        );
        let save_target = self.step_inputs(
            self.step_named(self.job("validate"), "Save Cargo target cache"),
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
            let step = self.step_named(self.job("validate"), step_name);
            let uses = step
                .get(Value::String("uses".to_owned()))
                .map(value_as_str)
                .expect("cache step should declare a cache action");
            assert!(
                uses.starts_with(expected_uses),
                "`{step_name}` should use `{expected_uses}`",
            );
            assert!(
                inputs.contains_key(Value::String("key".to_owned())),
                "`{step_name}` should declare an explicit cache key",
            );
        }

        assert_eq!(
            restore_registry
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("~/.cargo/registry/cache\n~/.cargo/registry/index\n~/.cargo/git/db\n"),
            "validation should restore the Cargo registry and git dependency caches",
        );
        assert_eq!(
            restore_target
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("target"),
            "validation should restore the shared workspace target cache",
        );
        assert_eq!(
            save_registry
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("~/.cargo/registry/cache\n~/.cargo/registry/index\n~/.cargo/git/db\n"),
            "validation should save the Cargo registry and git dependency caches",
        );
        assert_eq!(
            save_target
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("target"),
            "validation should save the shared workspace target cache",
        );
        assert!(
            restore_registry
                .get(Value::String("key".to_owned()))
                .map(value_as_str)
                .is_some_and(|key| key.contains("validate-cargo-registry")),
            "validation registry cache key should stay explicit and stable",
        );
        assert!(
            restore_target
                .get(Value::String("key".to_owned()))
                .map(value_as_str)
                .is_some_and(|key| key.contains("validate-target")),
            "validation target cache key should stay explicit and stable",
        );
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
            env.get(Value::String("REGISTRY".to_owned())).map(value_as_str),
            Some(PublishedImageContract::registry_host()),
            "workflow should define GHCR once through a shared REGISTRY env boundary",
        );
        assert_eq!(
            self.workflow_text.matches("ghcr.io").count(),
            1,
            "workflow should not scatter raw GHCR coordinates across multiple steps",
        );

        for image in PublishedImageContract::all() {
            let target = ImageBuildTargetContract::find(image.image_id());
            let expected_repository = format!(
                "${{{{ github.repository_owner }}}}/{}",
                image.repository()
            );
            assert_eq!(
                env.get(Value::String(target.repository_env().to_owned()))
                    .map(value_as_str),
                Some(expected_repository.as_str()),
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
            publish_script.contains("--platform linux/amd64,linux/arm64"),
            "publish-image job must publish both amd64 and arm64 images",
        );
        assert!(
            publish_script.contains("${{ env.REGISTRY }}/"),
            "publish-image job must tag GHCR through the shared registry boundary",
        );
        assert!(
            publish_script.contains("${{ github.sha }}"),
            "publish-image job must tag only the pushed commit SHA",
        );
        assert!(
            publish_script.contains("--push"),
            "publish-image job must push after a successful build",
        );

        for forbidden_marker in ["latest", "refs/tags/", "github.ref_name", "type=semver"] {
            assert!(
                !publish_script.contains(forbidden_marker),
                "publish-image job must not introduce `{forbidden_marker}` tags",
            );
        }
    }

    pub fn assert_installs_publish_dependencies_via_direct_shell_steps(&self) {
        let validate_install = self.step_run_script(
            self.step_named(self.job("validate"), "Install Rust toolchain"),
            "Install Rust toolchain",
        );
        assert!(
            validate_install.contains("rustup toolchain install 1.93.0"),
            "validation should install the pinned Rust toolchain directly via shell",
        );
        assert!(
            validate_install.contains("rustup default 1.93.0"),
            "validation should activate the pinned Rust toolchain directly via shell",
        );
        assert!(
            validate_install.contains("sudo apt-get install --yes postgresql"),
            "validation should install PostgreSQL tooling needed by the contract tests",
        );
        assert!(
            validate_install.contains("find /usr/lib/postgresql"),
            "validation should discover the installed PostgreSQL bin directory explicitly",
        );
        assert!(
            validate_install.contains("GITHUB_PATH"),
            "validation should add PostgreSQL binaries to PATH for later test steps",
        );

        let publish_install = self.step_run_script(
            self.step_named(self.job("publish-image"), "Install publish dependencies"),
            "Install publish dependencies",
        );
        for required_marker in [
            "sudo apt-get update",
            "sudo apt-get install --yes binfmt-support qemu-user-static",
            "sudo update-binfmts --enable qemu-aarch64",
            "curl -fsSL",
            "docker buildx version",
            "docker buildx create --name publish-builder --driver docker-container --use",
            "docker buildx inspect --bootstrap",
        ] {
            assert!(
                publish_install.contains(required_marker),
                "publish dependency installation must include `{required_marker}`",
            );
        }

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

    pub fn assert_proves_multi_arch_builder_support_before_publishing(&self) {
        let publish_job = self.job("publish-image");
        let probe_script = self.step_run_script(
            self.step_named(publish_job, "Assert multi-arch builder support"),
            "Assert multi-arch builder support",
        );
        for required_marker in [
            "docker buildx inspect --bootstrap",
            "buildx_platforms",
            "linux/amd64",
            "linux/arm64",
            "grep -F",
        ] {
            assert!(
                probe_script.contains(required_marker),
                "multi-arch builder proof step must include `{required_marker}`",
            );
        }

        let publish_script =
            self.step_run_script(self.step_named(publish_job, "Publish image"), "Publish image");
        assert!(
            publish_script.contains("--progress plain"),
            "publish-image job must use plain buildx progress so hosted failures stay inspectable",
        );
    }

    pub fn assert_emits_published_image_manifest_for_downstream_consumers(&self) {
        let manifest_job = self.job("publish-manifest");
        let manifest_env = self
            .job_env("publish-manifest")
            .expect("publish-manifest job should define an env mapping");
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
            Some("${{ matrix.artifact_name }}"),
            "publish-image job should upload one artifact per target through the shared matrix metadata",
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
            assert!(
                manifest_script.contains(target.manifest_key()),
                "manifest step should persist `{}` for downstream consumers",
                target.manifest_key(),
            );
        }
    }

    pub fn assert_masks_derived_sensitive_values_and_never_logs_raw_credentials(&self) {
        let mask_script = self.step_run_script(
            self.step_named(self.job("publish-image"), "Mask derived publish credentials"),
            "Mask derived publish credentials",
        );
        assert!(
            mask_script.contains("derived_registry_auth"),
            "workflow should derive a non-secret composite credential for masking verification",
        );
        assert!(
            mask_script.contains("::add-mask::${derived_registry_auth}"),
            "workflow should explicitly mask the derived credential before any output",
        );
        assert!(
            mask_script.contains("derived registry auth (masked)"),
            "workflow should print a masked diagnostic so hosted logs can prove redaction works",
        );

        let login_script = self.step_run_script(
            self.step_named(self.job("publish-image"), "Login to registry"),
            "Login to registry",
        );
        assert!(
            login_script.contains("docker login"),
            "publish-image job should log in to the registry explicitly through shell",
        );
        assert!(
            login_script.contains("--password-stdin"),
            "publish-image job must pass credentials through stdin instead of echoing them into the command line",
        );
        assert!(
            !login_script.contains("--password "),
            "publish-image job must not pass credentials as a shell argument",
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
            Some("ubuntu-latest"),
            "publish-image job should run on the explicit hosted runner boundary until a trusted native arm64 runner exists",
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
            "publish-image matrix should cover the canonical image target set exactly once",
        );
        assert_eq!(
            self.job_needs_names("publish-manifest"),
            vec!["publish-image"],
            "publish-manifest should aggregate after the parallel image publication completes",
        );
    }

    pub fn assert_publish_jobs_use_remote_buildkit_caches(&self) {
        let publish_script = self.step_run_script(
            self.step_named(self.job("publish-image"), "Publish image"),
            "Publish image",
        );
        assert!(
            publish_script.contains("--cache-from \"type=gha,scope=${{ matrix.cache_scope }}\""),
            "publish-image job must restore BuildKit cache state from the matrix-defined cache scope",
        );
        assert!(
            publish_script.contains(
                "--cache-to \"type=gha,scope=${{ matrix.cache_scope }},mode=max\""
            ),
            "publish-image job must save BuildKit cache state back to the matrix-defined cache scope",
        );

        for target in ImageBuildTargetContract::all() {
            assert!(
                !target.cache_scope().is_empty(),
                "shared build target `{}` must define a non-empty cache scope",
                target.image_id(),
            );
        }
    }

    pub fn assert_arm64_strategy_is_explicit(&self) {
        let publish_env = self
            .job_env("publish-image")
            .expect("publish-image job should define an env mapping");
        assert_eq!(
            publish_env
                .get(Value::String("ARM64_BUILD_STRATEGY".to_owned()))
                .map(value_as_str),
            Some("emulated-buildx-qemu"),
            "publish-image job must record the current arm64 strategy explicitly",
        );

        let strategy_step = self.step_run_script(
            self.step_named(self.job("publish-image"), "Record arm64 strategy decision"),
            "Record arm64 strategy decision",
        );
        assert!(
            strategy_step.contains("No trusted native arm64 runner label is configured in-repo"),
            "workflow should explicitly record why the native arm64 path is currently rejected",
        );
        assert!(
            strategy_step.contains("emulated buildx path explicit"),
            "workflow should document that the current arm64 path remains the explicit emulated buildx strategy",
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

    fn job(&self, name: &str) -> &Mapping {
        self.document
            .as_mapping()
            .expect("workflow document should be a mapping")
            .get(Value::String("jobs".to_owned()))
            .and_then(Value::as_mapping)
            .and_then(|jobs| jobs.get(Value::String(name.to_owned())))
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

    fn publish_matrix_entries(&self) -> &[Value] {
        self.job("publish-image")
            .get(Value::String("strategy".to_owned()))
            .and_then(Value::as_mapping)
            .and_then(|strategy| strategy.get(Value::String("matrix".to_owned())))
            .and_then(Value::as_mapping)
            .and_then(|matrix| matrix.get(Value::String("include".to_owned())))
            .and_then(Value::as_sequence)
            .map(Vec::as_slice)
            .expect("publish-image job should define a matrix.include sequence")
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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}
