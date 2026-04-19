use std::{fs, path::PathBuf};

use serde_yaml::{Mapping, Value};

pub struct GithubWorkflowContract {
    workflow_text: String,
    document: Value,
}

impl GithubWorkflowContract {
    pub fn load_master_image() -> Self {
        let workflow_path = repo_root().join(".github/workflows/master-image.yml");
        let workflow_text = fs::read_to_string(&workflow_path).unwrap_or_else(|error| {
            panic!(
                "master image workflow `{}` should be readable: {error}",
                workflow_path.display()
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
            document,
        }
    }

    pub fn assert_pushes_to_master_only(&self) {
        let workflow_on = self.workflow_on();
        let push = workflow_on
            .get(Value::String("push".to_owned()))
            .and_then(Value::as_mapping)
            .expect("workflow `on.push` should be a mapping");
        let branches = push
            .get(Value::String("branches".to_owned()))
            .and_then(Value::as_sequence)
            .expect("workflow `on.push.branches` should be a sequence");
        let actual_branches = branches
            .iter()
            .map(value_as_str)
            .collect::<Vec<_>>();

        assert_eq!(
            actual_branches,
            vec!["master"],
            "workflow must trigger only on pushes to `master`",
        );
        assert!(
            !workflow_on.contains_key(Value::String("pull_request".to_owned())),
            "workflow must not define a pull_request trigger",
        );
    }

    pub fn assert_runs_validation_commands(&self, expected_commands: &[&str]) {
        let run_commands = self.run_commands();

        for expected_command in expected_commands {
            assert!(
                run_commands.iter().any(|command| {
                    command
                        .lines()
                        .any(|line| line.trim() == *expected_command)
                }),
                "workflow must run `{expected_command}` as part of repository validation",
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
        let registry = login_step
            .get(Value::String("with".to_owned()))
            .and_then(Value::as_mapping)
            .and_then(|with| with.get(Value::String("registry".to_owned())))
            .map(value_as_str)
            .expect("publish job should log in to a registry");
        assert_eq!(
            registry, "${{ env.REGISTRY }}",
            "publish job should read registry coordinates from the shared env boundary",
        );

        let build_step = self.step_using_prefix(publish_job, "docker/build-push-action");
        let with = build_step
            .get(Value::String("with".to_owned()))
            .and_then(Value::as_mapping)
            .expect("build-push step should define with inputs");
        let push = with
            .get(Value::String("push".to_owned()))
            .expect("build-push step should set push");
        assert!(
            value_as_bool(push),
            "build-push step must push the validated image to GHCR",
        );
        let tags = with
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
        let tags = build_step
            .get(Value::String("with".to_owned()))
            .and_then(Value::as_mapping)
            .and_then(|with| with.get(Value::String("tags".to_owned())))
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

    fn step_using_prefix<'a>(&self, job: &'a Mapping, uses_prefix: &str) -> &'a Mapping {
        job.get(Value::String("steps".to_owned()))
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
            .filter_map(Value::as_mapping)
            .find(|step| {
                step.get(Value::String("uses".to_owned()))
                    .and_then(Value::as_str)
                    .is_some_and(|uses| uses.starts_with(uses_prefix))
            })
            .unwrap_or_else(|| panic!("workflow job should use `{uses_prefix}`"))
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
