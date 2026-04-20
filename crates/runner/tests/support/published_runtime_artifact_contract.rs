use std::{
    path::PathBuf,
};

pub struct PublishedRuntimeArtifactSpec {
    image_id: &'static str,
    repository: &'static str,
    readme_image_env: &'static str,
    compose_artifact_file: &'static str,
    compose_service_name: &'static str,
}

impl PublishedRuntimeArtifactSpec {
    pub const fn new(
        image_id: &'static str,
        repository: &'static str,
        readme_image_env: &'static str,
        compose_artifact_file: &'static str,
        compose_service_name: &'static str,
    ) -> Self {
        Self {
            image_id,
            repository,
            readme_image_env,
            compose_artifact_file,
            compose_service_name,
        }
    }

    pub fn image_id(&self) -> &'static str {
        self.image_id
    }

    pub fn repository(&self) -> &'static str {
        self.repository
    }

    pub fn readme_image_env(&self) -> &'static str {
        self.readme_image_env
    }

    pub fn compose_artifact_file(&self) -> &'static str {
        self.compose_artifact_file
    }

    pub fn compose_service_name(&self) -> &'static str {
        self.compose_service_name
    }
}

const PUBLISHED_RUNTIME_ARTIFACTS: [PublishedRuntimeArtifactSpec; 3] = [
    PublishedRuntimeArtifactSpec::new(
        "runner",
        "cockroach-migrate-runner",
        "RUNNER_IMAGE",
        "runner.compose.yml",
        "runner",
    ),
    PublishedRuntimeArtifactSpec::new(
        "setup-sql",
        "cockroach-migrate-setup-sql",
        "SETUP_SQL_IMAGE",
        "setup-sql.compose.yml",
        "setup-sql",
    ),
    PublishedRuntimeArtifactSpec::new(
        "verify",
        "cockroach-migrate-verify",
        "VERIFY_IMAGE",
        "verify.compose.yml",
        "verify",
    ),
];

pub struct PublishedRuntimeArtifactContract;

impl PublishedRuntimeArtifactContract {
    pub fn compose_artifact_dir() -> PathBuf {
        repo_root().join("artifacts").join("compose")
    }

    pub fn compose_artifact_path(image_id: &str) -> PathBuf {
        Self::compose_artifact_dir().join(Self::find(image_id).compose_artifact_file())
    }

    pub fn all() -> &'static [PublishedRuntimeArtifactSpec] {
        &PUBLISHED_RUNTIME_ARTIFACTS
    }

    pub fn find(image_id: &str) -> &'static PublishedRuntimeArtifactSpec {
        Self::all()
            .iter()
            .find(|artifact| artifact.image_id() == image_id)
            .unwrap_or_else(|| panic!("unknown published runtime artifact `{image_id}`"))
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
