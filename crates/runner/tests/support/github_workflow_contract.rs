use std::{fs, path::PathBuf};

use serde_yaml::{Mapping, Value};

use crate::published_image_contract_support::{PublishedImageContract, PublishedImageSpec};

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
            .job_permissions("publish")
            .expect("publish job should define explicit permissions");
        assert_eq!(
            publish_permissions
                .get(Value::String("contents".to_owned()))
                .map(value_as_str),
            Some("read"),
            "publish job should retain read access for checkout",
        );
        assert_eq!(
            publish_permissions
                .get(Value::String("packages".to_owned()))
                .map(value_as_str),
            Some("write"),
            "publish job should be the only job with package publish permission",
        );

        assert!(
            self.job_permissions("validate").is_none(),
            "validate job must not elevate permissions",
        );

        for job_name in ["validate", "publish"] {
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
        let publish_job = self.job("publish");
        let publish_condition = publish_job
            .get(Value::String("if".to_owned()))
            .map(value_as_str)
            .expect("publish job should define an explicit trust gate");
        assert!(
            publish_condition.contains("github.event_name == 'push'"),
            "publish job trust gate must require the push event",
        );
        assert!(
            publish_condition.contains("github.ref == 'refs/heads/main'"),
            "publish job trust gate must require the main branch ref",
        );

        let publish_needs = publish_job
            .get(Value::String("needs".to_owned()))
            .map(value_as_str)
            .expect("publish job should depend on repository validation");
        assert_eq!(
            publish_needs, "validate",
            "publish job must wait for validation to pass",
        );

        let checkout_inputs = self.step_inputs(
            self.step_using_prefix(publish_job, "actions/checkout"),
            "actions/checkout",
        );
        assert!(
            !checkout_inputs.contains_key(Value::String("ref".to_owned())),
            "publish checkout must not override the trusted pushed commit ref",
        );
    }

    pub fn assert_ci_publish_safety_model_is_documented(&self) {
        assert!(
            self.readme_text.contains("## CI Publish Safety"),
            "README should document the CI publish safety model",
        );

        for required_line in [
            "Random pull requests, forks, `pull_request_target`, manual dispatch, reusable workflow calls, scheduled runs, tag pushes, issue-triggered events, and release events do not trigger the protected image-publish workflow.",
            "The `publish` job still carries an explicit `if:` gate that requires a `push` event on `refs/heads/main`, so widening workflow triggers later does not silently open the release path.",
            "Only the `publish` job gets `packages: write`, checkout disables credential persistence, derived registry credentials are masked before any diagnostic output, and the pushed images are tagged only with `${{ github.sha }}` from the validated commit.",
            "Validation runs the repository lint and default test gates before publish, each image is pushed for both `linux/amd64` and `linux/arm64`, and the workflow emits a published-image manifest that downstream registry-only checks can consume directly.",
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
            let target = workflow_publish_target(image.image_id());
            let expected_repository = format!(
                "${{{{ github.repository_owner }}}}/{}",
                image.repository()
            );
            assert_eq!(
                env.get(Value::String(target.repository_env.to_owned()))
                    .map(value_as_str),
                Some(expected_repository.as_str()),
                "workflow should define `{}` through the shared image contract",
                target.repository_env,
            );

            let publish_step = self.step_named(self.job("publish"), &publish_step_name(image));
            let publish_script = self.step_run_script(publish_step, &publish_step_name(image));
            assert!(
                publish_script.contains(target.dockerfile),
                "publish step for `{}` should build the canonical Dockerfile `{}`",
                image.image_id(),
                target.dockerfile,
            );
            assert!(
                publish_script.lines().any(|line| line.trim() == target.context),
                "publish step for `{}` should build the canonical context `{}`",
                image.image_id(),
                target.context,
            );
            assert!(
                publish_script.contains(&format!("${{{{ env.{} }}}}", target.repository_env)),
                "publish step for `{}` should tag through `{}`",
                image.image_id(),
                target.repository_env,
            );
        }
    }

    pub fn assert_uses_multi_arch_commit_sha_tags_only(&self) {
        for image in PublishedImageContract::all() {
            let target = workflow_publish_target(image.image_id());
            let publish_script = self.step_run_script(
                self.step_named(self.job("publish"), &publish_step_name(image)),
                &publish_step_name(image),
            );
            assert!(
                publish_script.contains("docker buildx build"),
                "publish step for `{}` must use `docker buildx build`",
                image.image_id(),
            );
            assert!(
                publish_script.contains("--platform linux/amd64,linux/arm64"),
                "publish step for `{}` must publish both amd64 and arm64 images",
                image.image_id(),
            );
            assert!(
                publish_script.contains("${{ env.REGISTRY }}/"),
                "publish step for `{}` must tag GHCR through the shared registry boundary",
                image.image_id(),
            );
            assert!(
                publish_script.contains(&format!("${{{{ env.{} }}}}", target.repository_env)),
                "publish step for `{}` must tag through `{}`",
                image.image_id(),
                target.repository_env,
            );
            assert!(
                publish_script.contains("${{ github.sha }}"),
                "publish step for `{}` must tag only the pushed commit SHA",
                image.image_id(),
            );
            assert!(
                publish_script.contains("--push"),
                "publish step for `{}` must push after a successful build",
                image.image_id(),
            );

            for forbidden_marker in ["latest", "refs/tags/", "github.ref_name", "type=semver"] {
                assert!(
                    !publish_script.contains(forbidden_marker),
                    "publish step for `{}` must not introduce `{forbidden_marker}` tags",
                    image.image_id(),
                );
            }
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

        let publish_install = self.step_run_script(
            self.step_named(self.job("publish"), "Install publish dependencies"),
            "Install publish dependencies",
        );
        for required_marker in [
            "sudo apt-get update",
            "sudo apt-get install --yes jq qemu-user-static",
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

    pub fn assert_emits_published_image_manifest_for_downstream_consumers(&self) {
        let publish_job = self.job("publish");
        let publish_env = publish_job
            .get(Value::String("env".to_owned()))
            .and_then(Value::as_mapping)
            .expect("publish job should define a scoped env mapping");
        let outputs = publish_job
            .get(Value::String("outputs".to_owned()))
            .and_then(Value::as_mapping)
            .expect("publish job should expose outputs for downstream consumers");
        assert_eq!(
            outputs
                .get(Value::String("publish_manifest".to_owned()))
                .map(value_as_str),
            Some("${{ steps.publish-manifest.outputs.publish_manifest }}"),
            "publish job should expose the manifest output",
        );

        let manifest_step =
            self.step_named(publish_job, "Publish manifest");
        let manifest_script = self.step_run_script(manifest_step, "Publish manifest");
        assert_eq!(
            publish_env
                .get(Value::String("PUBLISHED_IMAGE_MANIFEST".to_owned()))
                .map(value_as_str),
            Some("${{ runner.temp }}/published-images.json"),
            "publish job should scope the manifest path to the selected runner",
        );
        assert!(
            manifest_script.contains("${{ env.PUBLISHED_IMAGE_MANIFEST }}"),
            "manifest step should write the published image manifest through the publish job env boundary",
        );
        assert!(
            manifest_script.contains("GITHUB_OUTPUT"),
            "manifest step should expose published image refs through job outputs",
        );

        for image in PublishedImageContract::all() {
            let target = workflow_publish_target(image.image_id());
            let expected_output = target.output_expression;
            assert_eq!(
                outputs
                    .get(Value::String(target.manifest_key.to_owned()))
                    .map(value_as_str),
                Some(expected_output.as_str()),
                "publish job should expose `{}` through the manifest step",
                target.manifest_key,
            );
            assert!(
                manifest_script.contains(target.manifest_key),
                "manifest step should persist `{}` for downstream consumers",
                target.manifest_key,
            );
        }

        let upload_inputs = self.step_inputs(
            self.step_using_prefix(publish_job, "actions/upload-artifact"),
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
    }

    pub fn assert_masks_derived_sensitive_values_and_never_logs_raw_credentials(&self) {
        let mask_script = self.step_run_script(
            self.step_named(self.job("publish"), "Mask derived publish credentials"),
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
            self.step_named(self.job("publish"), "Login to registry"),
            "Login to registry",
        );
        assert!(
            login_script.contains("docker login"),
            "publish job should log in to the registry explicitly through shell",
        );
        assert!(
            login_script.contains("--password-stdin"),
            "publish job must pass credentials through stdin instead of echoing them into the command line",
        );
        assert!(
            !login_script.contains("--password "),
            "publish job must not pass credentials as a shell argument",
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

    fn job_permissions(&self, name: &str) -> Option<&Mapping> {
        self.job(name)
            .get(Value::String("permissions".to_owned()))
            .and_then(Value::as_mapping)
    }

    fn job_run_scripts(&self, name: &str) -> Vec<&str> {
        self.steps(self.job(name))
            .iter()
            .filter_map(Value::as_mapping)
            .filter_map(|step| step.get(Value::String("run".to_owned())))
            .map(value_as_str)
            .collect()
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
            .unwrap_or_else(|| {
                panic!("workflow step `{step_name}` should define explicit inputs")
            })
    }

    fn step_run_script<'a>(&self, step: &'a Mapping, step_name: &str) -> &'a str {
        step.get(Value::String("run".to_owned()))
            .map(value_as_str)
            .unwrap_or_else(|| panic!("workflow step `{step_name}` should define a run script"))
    }
}

fn publish_step_name(image: &PublishedImageSpec) -> String {
    format!("Publish {} image", image.image_id())
}

struct WorkflowPublishTarget {
    repository_env: &'static str,
    dockerfile: &'static str,
    context: &'static str,
    manifest_key: &'static str,
    output_expression: String,
}

fn workflow_publish_target(image_id: &str) -> WorkflowPublishTarget {
    match image_id {
        "runner" => WorkflowPublishTarget {
            repository_env: "RUNNER_IMAGE_REPOSITORY",
            dockerfile: "./Dockerfile",
            context: ".",
            manifest_key: "runner_image_ref",
            output_expression: "${{ steps.publish-manifest.outputs.runner_image_ref }}".to_owned(),
        },
        "setup-sql" => WorkflowPublishTarget {
            repository_env: "SETUP_SQL_IMAGE_REPOSITORY",
            dockerfile: "./crates/setup-sql/Dockerfile",
            context: ".",
            manifest_key: "setup_sql_image_ref",
            output_expression:
                "${{ steps.publish-manifest.outputs.setup_sql_image_ref }}".to_owned(),
        },
        "verify" => WorkflowPublishTarget {
            repository_env: "VERIFY_IMAGE_REPOSITORY",
            dockerfile: "./cockroachdb_molt/molt/Dockerfile",
            context: "./cockroachdb_molt/molt",
            manifest_key: "verify_image_ref",
            output_expression: "${{ steps.publish-manifest.outputs.verify_image_ref }}".to_owned(),
        },
        _ => panic!("unknown workflow publish target `{image_id}`"),
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
