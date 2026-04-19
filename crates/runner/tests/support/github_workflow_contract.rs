use std::{fs, path::PathBuf};

use serde_yaml::{Mapping, Value};

pub struct GithubWorkflowContract {
    workflow_text: String,
    readme_text: String,
    document: Value,
}

impl GithubWorkflowContract {
    pub fn load_master_image() -> Self {
        let workflow_path = repo_root().join(".github/workflows/master-image.yml");
        let readme_path = repo_root().join("README.md");
        let workflow_text = fs::read_to_string(&workflow_path).unwrap_or_else(|error| {
            panic!(
                "master image workflow `{}` should be readable: {error}",
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
                "master image workflow `{}` should parse as YAML: {error}",
                workflow_path.display()
            )
        });

        Self {
            workflow_text,
            readme_text,
            document,
        }
    }

    pub fn assert_pushes_to_master_only(&self) {
        assert_eq!(
            self.push_branches(),
            vec!["master"],
            "workflow must trigger only on pushes to `master`",
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
            vec!["master"],
            "workflow must trigger only on pushes to `master`",
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
        ] {
            assert!(
                !self
                    .workflow_on()
                    .contains_key(Value::String(forbidden_trigger.to_owned())),
                "workflow must not define a `{forbidden_trigger}` trigger",
            );
        }
    }

    pub fn assert_runs_validation_commands(&self, expected_commands: &[&str]) {
        let run_commands = self.run_commands();

        for expected_command in expected_commands {
            assert!(
                run_commands.iter().any(|command| {
                    command.lines().any(|line| line.trim() == *expected_command)
                }),
                "workflow must run `{expected_command}` as part of repository validation",
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

        let validate_permissions = self.job_permissions("validate");
        assert!(
            validate_permissions.is_none()
                || !validate_permissions
                    .expect("validate permissions presence already checked")
                    .contains_key(Value::String("packages".to_owned())),
            "validate job must not have package publish permission",
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
    }

    pub fn assert_publish_is_explicitly_gated_to_the_trusted_master_push_commit(&self) {
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
            publish_condition.contains("github.ref == 'refs/heads/master'"),
            "publish job trust gate must require the master branch ref",
        );
        assert!(
            !publish_job.contains_key(Value::String("uses".to_owned())),
            "publish job must not delegate publishing through a reusable workflow",
        );

        let checkout_inputs = self.step_inputs(
            self.step_using_prefix(publish_job, "actions/checkout"),
            "checkout",
        );
        assert!(
            !checkout_inputs.contains_key(Value::String("ref".to_owned())),
            "publish checkout must not override the trusted pushed commit ref",
        );

        let build_inputs = self.step_inputs(
            self.step_using_prefix(publish_job, "docker/build-push-action"),
            "docker/build-push-action",
        );
        assert_eq!(
            build_inputs
                .get(Value::String("context".to_owned()))
                .map(value_as_str),
            Some("."),
            "publish job must build from the repository root of the trusted checked-out commit",
        );
        assert_eq!(
            build_inputs
                .get(Value::String("tags".to_owned()))
                .map(value_as_str),
            Some("${{ env.REGISTRY }}/${{ env.IMAGE_REPOSITORY }}:${{ github.sha }}"),
            "publish job must tag the real image only with the trusted pushed commit SHA",
        );
    }

    pub fn assert_ci_publish_safety_model_is_documented(&self) {
        assert!(
            self.readme_text.contains("## CI Publish Safety"),
            "README should document the CI publish safety model",
        );
        for required_line in [
            "Random pull requests, forks, `pull_request_target`, manual dispatch, reusable workflow calls, scheduled runs, and tag pushes do not trigger the protected image-publish workflow.",
            "The `publish` job still carries an explicit `if:` gate that requires a `push` event on `refs/heads/master`, so widening workflow triggers later does not silently open the release path.",
            "Only the `publish` job gets `packages: write`, checkout disables credential persistence, and the pushed image is tagged only with `${{ github.sha }}` from the validated commit.",
            "Before any push, the workflow builds one release-image archive, scans that exact archive with Trivy, fails on `HIGH` or `CRITICAL` findings, and always uploads the scan report artifact for review.",
        ] {
            assert!(
                self.readme_text.contains(required_line),
                "README should explain CI publish safety invariant: {required_line}",
            );
        }
    }

    pub fn assert_commit_tagged_ghcr_publish_only(&self) {
        let publish_job = self.job("publish");
        let publish_needs = publish_job
            .get(Value::String("needs".to_owned()))
            .map(value_as_str)
            .expect("publish job should depend on the validation job");
        assert_eq!(
            publish_needs, "validate",
            "publish job must wait for the validation job",
        );

        let login_step = self.step_using_prefix(publish_job, "docker/login-action");
        let registry = self
            .step_inputs(login_step, "docker/login-action")
            .get(Value::String("registry".to_owned()))
            .map(value_as_str)
            .expect("publish job should log in to a registry");
        assert_eq!(
            registry, "${{ env.REGISTRY }}",
            "publish job should read registry coordinates from the shared env boundary",
        );

        let build_step = self.step_using_prefix(publish_job, "docker/build-push-action");
        let build_inputs = self.step_inputs(build_step, "docker/build-push-action");
        let tags = build_inputs
            .get(Value::String("tags".to_owned()))
            .map(value_as_str)
            .expect("build-push step should define tags");
        assert!(
            tags.contains("${{ env.REGISTRY }}/"),
            "published image must target GHCR through the shared registry boundary",
        );
        assert!(
            tags.contains("${{ github.sha }}"),
            "published image tag must be derived from the pushed commit SHA",
        );
        for forbidden_tag in ["latest", "type=semver", "refs/tags/", "github.ref_name"] {
            assert!(
                !tags.contains(forbidden_tag),
                "published image tags must not include `{forbidden_tag}`",
            );
        }

        let push_script = self.run_script_containing(
            publish_job,
            "docker push \"${{ env.REGISTRY }}/${{ env.IMAGE_REPOSITORY }}:${{ github.sha }}\"",
        );
        assert!(
            push_script.contains("${{ github.sha }}"),
            "publish command must push only the trusted commit SHA image tag",
        );
    }

    pub fn assert_scans_the_release_archive_before_publishing(&self) {
        let publish_job = self.job("publish");
        let archive_path = self
            .workflow_env()
            .get(Value::String("RELEASE_IMAGE_ARCHIVE".to_owned()))
            .map(value_as_str)
            .expect("workflow should define a shared RELEASE_IMAGE_ARCHIVE env");
        let publish_steps = self.steps(publish_job);

        let build_step = self.step_using_prefix(publish_job, "docker/build-push-action");
        let build_inputs = self.step_inputs(build_step, "docker/build-push-action");
        assert_eq!(
            build_inputs
                .get(Value::String("push".to_owned()))
                .map(value_as_bool),
            Some(false),
            "publish job must build the release image archive before the scan gate pushes it",
        );
        assert_eq!(
            build_inputs
                .get(Value::String("outputs".to_owned()))
                .map(value_as_str),
            Some("type=docker,dest=${{ env.RELEASE_IMAGE_ARCHIVE }}"),
            "publish job must export the release image once as a docker archive tarball",
        );

        let build_index = self.step_index_using_prefix(publish_job, "docker/build-push-action");
        let scan_index = self.step_index_with_run_containing(
            publish_job,
            "trivy image --input \"${{ env.RELEASE_IMAGE_ARCHIVE }}\"",
        );
        let load_index = self.step_index_with_run_containing(
            publish_job,
            "docker load --input \"${{ env.RELEASE_IMAGE_ARCHIVE }}\"",
        );
        let push_index = self.step_index_with_run_containing(
            publish_job,
            "docker push \"${{ env.REGISTRY }}/${{ env.IMAGE_REPOSITORY }}:${{ github.sha }}\"",
        );

        assert!(
            build_index < scan_index,
            "publish job must build the release archive before scanning it",
        );
        assert!(
            scan_index < load_index,
            "publish job must load the same scanned archive only after the scan passes",
        );
        assert!(
            load_index < push_index,
            "publish job must push only after loading the scanned archive",
        );

        let scan_step = publish_steps
            .get(scan_index)
            .and_then(Value::as_mapping)
            .expect("scan step should resolve");
        let scan_script = self.step_run_script(scan_step, "scan release archive");
        assert!(
            scan_script.contains("${{ env.RELEASE_IMAGE_ARCHIVE }}"),
            "scan step must read the release archive through the shared env boundary",
        );
        assert!(
            archive_path.ends_with("release-image.tar"),
            "release archive boundary should describe the built docker archive tarball",
        );
    }

    pub fn assert_release_scan_policy_is_explicit_and_visible(&self) {
        let publish_job = self.job("publish");
        let report_path = self
            .workflow_env()
            .get(Value::String("VULNERABILITY_REPORT".to_owned()))
            .map(value_as_str)
            .expect("workflow should define a shared VULNERABILITY_REPORT env");

        self.step_using_prefix(publish_job, "aquasecurity/setup-trivy");

        let scan_script = self.run_script_containing(
            publish_job,
            "trivy image --input \"${{ env.RELEASE_IMAGE_ARCHIVE }}\"",
        );
        for required_flag in [
            "--severity HIGH,CRITICAL",
            "--exit-code 1",
            "--format table",
            "--output \"${{ env.VULNERABILITY_REPORT }}\"",
        ] {
            assert!(
                scan_script.contains(required_flag),
                "scan step must define `{required_flag}` so the Trivy gate is explicit and actionable",
            );
        }

        let print_index = self
            .step_index_with_run_containing(publish_job, "cat \"${{ env.VULNERABILITY_REPORT }}\"");
        let print_step = self
            .steps(publish_job)
            .get(print_index)
            .and_then(Value::as_mapping)
            .expect("print report step should resolve");
        assert_eq!(
            print_step
                .get(Value::String("if".to_owned()))
                .map(value_as_str),
            Some("always()"),
            "report printing must still run when the vulnerability gate fails",
        );

        let upload_step = self.step_using_prefix(publish_job, "actions/upload-artifact");
        assert_eq!(
            upload_step
                .get(Value::String("if".to_owned()))
                .map(value_as_str),
            Some("always()"),
            "report upload must still run when the vulnerability gate fails",
        );
        let upload_inputs = self.step_inputs(upload_step, "actions/upload-artifact");
        assert_eq!(
            upload_inputs
                .get(Value::String("path".to_owned()))
                .map(value_as_str),
            Some("${{ env.VULNERABILITY_REPORT }}"),
            "report upload must preserve the shared report path boundary",
        );
        assert_eq!(
            upload_inputs
                .get(Value::String("if-no-files-found".to_owned()))
                .map(value_as_str),
            Some("error"),
            "report upload must fail loudly if the expected scan artifact is missing",
        );
        assert!(
            report_path.ends_with("vulnerability-report.txt"),
            "report boundary should describe the persisted vulnerability report artifact",
        );
    }

    pub fn assert_registry_coordinates_are_isolated(&self) {
        let env = self.workflow_env();
        assert_eq!(
            env.get(Value::String("REGISTRY".to_owned()))
                .map(value_as_str)
                .expect("workflow should define a shared REGISTRY env"),
            "ghcr.io",
            "workflow should define GHCR once through a shared REGISTRY env boundary",
        );
        assert!(
            env.contains_key(Value::String("IMAGE_REPOSITORY".to_owned())),
            "workflow should define a shared IMAGE_REPOSITORY env boundary",
        );
        assert_eq!(
            self.workflow_text.matches("ghcr.io").count(),
            1,
            "workflow should not scatter raw GHCR coordinates across multiple steps",
        );

        let publish_job = self.job("publish");
        let build_step = self.step_using_prefix(publish_job, "docker/build-push-action");
        let tags = self
            .step_inputs(build_step, "docker/build-push-action")
            .get(Value::String("tags".to_owned()))
            .map(value_as_str)
            .expect("build-push step should define tags");
        assert!(
            tags.contains("${{ env.IMAGE_REPOSITORY }}"),
            "build-push step should consume the shared IMAGE_REPOSITORY boundary",
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

    fn steps<'a>(&self, job: &'a Mapping) -> &'a [Value] {
        job.get(Value::String("steps".to_owned()))
            .and_then(Value::as_sequence)
            .map(Vec::as_slice)
            .expect("workflow job should define steps")
    }

    fn run_commands(&self) -> Vec<&str> {
        self.jobs()
            .values()
            .filter_map(Value::as_mapping)
            .flat_map(|job| {
                job.get(Value::String("steps".to_owned()))
                    .and_then(Value::as_sequence)
                    .into_iter()
                    .flatten()
            })
            .filter_map(Value::as_mapping)
            .filter_map(|step| step.get(Value::String("run".to_owned())))
            .map(value_as_str)
            .collect()
    }

    fn step_index_using_prefix(&self, job: &Mapping, uses_prefix: &str) -> usize {
        self.steps(job)
            .iter()
            .position(|step| {
                step.as_mapping()
                    .and_then(|mapping| mapping.get(Value::String("uses".to_owned())))
                    .and_then(Value::as_str)
                    .is_some_and(|uses| uses.starts_with(uses_prefix))
            })
            .unwrap_or_else(|| panic!("workflow job should use `{uses_prefix}`"))
    }

    fn step_index_with_run_containing(&self, job: &Mapping, snippet: &str) -> usize {
        self.steps(job)
            .iter()
            .position(|step| {
                step.as_mapping()
                    .and_then(|mapping| mapping.get(Value::String("run".to_owned())))
                    .and_then(Value::as_str)
                    .is_some_and(|run| run.contains(snippet))
            })
            .unwrap_or_else(|| {
                panic!("workflow job should define a run step containing `{snippet}`")
            })
    }

    fn run_script_containing<'a>(&self, job: &'a Mapping, snippet: &str) -> &'a str {
        self.steps(job)
            .iter()
            .filter_map(Value::as_mapping)
            .find_map(|step| {
                step.get(Value::String("run".to_owned()))
                    .and_then(Value::as_str)
                    .filter(|run| run.contains(snippet))
            })
            .unwrap_or_else(|| {
                panic!("workflow job should define a run step containing `{snippet}`")
            })
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

    fn step_inputs<'a>(&self, step: &'a Mapping, uses_prefix: &str) -> &'a Mapping {
        step.get(Value::String("with".to_owned()))
            .and_then(Value::as_mapping)
            .unwrap_or_else(|| {
                panic!("workflow step `{uses_prefix}` should define explicit inputs")
            })
    }

    fn step_run_script<'a>(&self, step: &'a Mapping, step_name: &str) -> &'a str {
        step.get(Value::String("run".to_owned()))
            .map(value_as_str)
            .unwrap_or_else(|| panic!("workflow step `{step_name}` should define a run script"))
    }

    fn jobs(&self) -> &Mapping {
        self.document
            .as_mapping()
            .expect("workflow document should be a mapping")
            .get(Value::String("jobs".to_owned()))
            .and_then(Value::as_mapping)
            .expect("workflow should define jobs")
    }

    fn workflow_env(&self) -> &Mapping {
        self.document
            .as_mapping()
            .expect("workflow document should be a mapping")
            .get(Value::String("env".to_owned()))
            .and_then(Value::as_mapping)
            .expect("workflow should define a shared env mapping")
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
