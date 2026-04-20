use std::{collections::BTreeSet, fs, path::Path};

use serde_yaml::{Mapping, Value};

use crate::published_runtime_artifact_contract_support::PublishedRuntimeArtifactContract;

pub struct ComposeArtifactContract {
    image_id: String,
    document: Value,
}

impl ComposeArtifactContract {
    pub fn assert_defines_three_dedicated_compose_artifacts() {
        let compose_dir = PublishedRuntimeArtifactContract::compose_artifact_dir();
        let actual_files = read_compose_artifact_files(&compose_dir);
        let expected_files = PublishedRuntimeArtifactContract::all()
            .iter()
            .map(|artifact| artifact.compose_artifact_file().to_owned())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            actual_files, expected_files,
            "published runtime contract must define exactly three dedicated compose artifacts under `{}`",
            compose_dir.display(),
        );
    }

    pub fn load(image_id: &str) -> Self {
        let compose_path = PublishedRuntimeArtifactContract::compose_artifact_path(image_id);
        let text = fs::read_to_string(&compose_path).unwrap_or_else(|error| {
            panic!(
                "compose artifact `{}` should be readable: {error}",
                compose_path.display()
            )
        });
        let document = serde_yaml::from_str(&text).unwrap_or_else(|error| {
            panic!(
                "compose artifact `{}` should parse as YAML: {error}",
                compose_path.display()
            )
        });

        Self {
            image_id: image_id.to_owned(),
            document,
        }
    }

    pub fn assert_runner_runtime_contract(&self) {
        self.assert_runtime_contract(
            &[("runner-config", "/config/runner.yml")],
            Some("${RUNNER_HTTPS_PORT:-8443}:8443"),
            &["run", "--config", "/config/runner.yml"],
        );
        for (config_name, file_path) in [
            ("runner-server-cert", "./config/certs/server.crt"),
            ("runner-server-key", "./config/certs/server.key"),
        ] {
            self.assert_service_config_target(config_name, &format!("/config/certs/{}", file_name(file_path)));
            self.assert_top_level_config_file(config_name, file_path);
        }
    }

    pub fn assert_setup_sql_runtime_contract(&self) {
        self.assert_runtime_contract(
            &[
                ("cockroach-setup-config", "/config/cockroach-setup.yml"),
                ("postgres-grants-config", "/config/postgres-grants.yml"),
                ("source-ca-cert", "/config/ca.crt"),
            ],
            None,
            &["emit-cockroach-sql", "--config", "/config/cockroach-setup.yml"],
        );
    }

    pub fn assert_verify_runtime_contract(&self) {
        self.assert_runtime_contract(
            &[
                ("verify-service-config", "/config/verify-service.yml"),
                ("verify-source-ca", "/config/certs/source-ca.crt"),
                ("verify-source-client-cert", "/config/certs/source-client.crt"),
                ("verify-source-client-key", "/config/certs/source-client.key"),
                ("verify-destination-ca", "/config/certs/destination-ca.crt"),
                ("verify-server-cert", "/config/certs/server.crt"),
                ("verify-server-key", "/config/certs/server.key"),
            ],
            Some("${VERIFY_HTTPS_PORT:-9443}:8080"),
            &["--config", "/config/verify-service.yml"],
        );
    }

    fn assert_runtime_contract(
        &self,
        expected_configs: &[(&str, &str)],
        expected_port: Option<&str>,
        expected_command: &[&str],
    ) {
        let runtime = PublishedRuntimeArtifactContract::find(&self.image_id);
        let service = self.service(runtime.compose_service_name());

        assert_eq!(
            self.services().len(),
            1,
            "compose artifact `{}` must define exactly one dedicated runtime service",
            runtime.compose_artifact_file(),
        );
        assert_eq!(
            service
                .get(Value::String("image".to_owned()))
                .map(value_as_str),
            Some(
                format!("${{{}}}", runtime.readme_image_env())
                .as_str()
            ),
            "compose artifact `{}` must use the published image coordinates",
            runtime.compose_artifact_file(),
        );
        assert_eq!(
            service
                .get(Value::String("command".to_owned()))
                .and_then(Value::as_sequence)
                .map(|command| command.iter().map(value_as_str).collect::<Vec<_>>()),
            Some(expected_command.to_vec()),
            "compose artifact `{}` must keep the dedicated runtime command surface",
            runtime.compose_artifact_file(),
        );

        match expected_port {
            Some(expected_port) => {
                assert_eq!(
                    service
                        .get(Value::String("ports".to_owned()))
                        .and_then(Value::as_sequence)
                        .map(|ports| ports.iter().map(value_as_str).collect::<Vec<_>>()),
                    Some(vec![expected_port]),
                    "compose artifact `{}` must expose the expected port contract",
                    runtime.compose_artifact_file(),
                );
            }
            None => {
                assert!(
                    !service.contains_key(Value::String("ports".to_owned())),
                    "compose artifact `{}` must not expose a long-running network port",
                    runtime.compose_artifact_file(),
                );
            }
        }

        for (config_name, target_path) in expected_configs {
            self.assert_service_config_target(config_name, target_path);
        }
    }

    fn services(&self) -> &Mapping {
        self.document
            .as_mapping()
            .expect("compose artifact should parse to a mapping")
            .get(Value::String("services".to_owned()))
            .and_then(Value::as_mapping)
            .expect("compose artifact should define `services`")
    }

    fn service(&self, service_name: &str) -> &Mapping {
        self.services()
            .get(Value::String(service_name.to_owned()))
            .and_then(Value::as_mapping)
            .unwrap_or_else(|| panic!("compose artifact should define service `{service_name}`"))
    }

    fn configs(&self) -> &Mapping {
        self.document
            .as_mapping()
            .expect("compose artifact should parse to a mapping")
            .get(Value::String("configs".to_owned()))
            .and_then(Value::as_mapping)
            .expect("compose artifact should define top-level `configs`")
    }

    fn assert_service_config_target(&self, config_name: &str, target_path: &str) {
        let runtime = PublishedRuntimeArtifactContract::find(&self.image_id);
        let configs = self
            .service(runtime.compose_service_name())
            .get(Value::String("configs".to_owned()))
            .and_then(Value::as_sequence)
            .expect("compose service should define `configs`");

        let config = configs
            .iter()
            .filter_map(Value::as_mapping)
            .find(|config| {
                config.get(Value::String("source".to_owned())).map(value_as_str) == Some(config_name)
            })
            .unwrap_or_else(|| {
                panic!(
                    "compose artifact `{}` must mount config `{config_name}`",
                    runtime.compose_artifact_file(),
                )
            });
        assert_eq!(
            config
                .get(Value::String("target".to_owned()))
                .map(value_as_str),
            Some(target_path),
            "compose artifact `{}` must mount `{config_name}` at `{target_path}`",
            runtime.compose_artifact_file(),
        );
    }

    fn assert_top_level_config_file(&self, config_name: &str, file_path: &str) {
        let runtime = PublishedRuntimeArtifactContract::find(&self.image_id);
        let config = self
            .configs()
            .get(Value::String(config_name.to_owned()))
            .and_then(Value::as_mapping)
            .unwrap_or_else(|| {
                panic!(
                    "compose artifact `{}` must define top-level config `{config_name}`",
                    runtime.compose_artifact_file(),
                )
            });
        assert_eq!(
            config.get(Value::String("file".to_owned())).map(value_as_str),
            Some(file_path),
            "compose artifact `{}` must source `{config_name}` from `{file_path}`",
            runtime.compose_artifact_file(),
        );
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .expect("config path should contain a filename")
}

fn read_compose_artifact_files(compose_dir: &Path) -> BTreeSet<String> {
    fs::read_dir(compose_dir)
        .unwrap_or_else(|error| {
            panic!(
                "published runtime compose artifact directory `{}` should be readable: {error}",
                compose_dir.display()
            )
        })
        .map(|entry| {
            let entry = entry.unwrap_or_else(|error| {
                panic!(
                    "published runtime compose artifact entry under `{}` should be readable: {error}",
                    compose_dir.display()
                )
            });
            entry
                .file_name()
                .into_string()
                .unwrap_or_else(|_| panic!("compose artifact filename should be valid UTF-8"))
        })
        .collect()
}

fn value_as_str(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("expected YAML string, got {value:?}"))
}
