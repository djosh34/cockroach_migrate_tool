pub struct ImageBuildTargetSpec {
    image_id: &'static str,
    repository_env: &'static str,
    dockerfile: &'static str,
    context: &'static str,
    manifest_key: &'static str,
    artifact_name: &'static str,
    cache_scope: &'static str,
    build_kind: &'static str,
}

impl ImageBuildTargetSpec {
    pub fn image_id(&self) -> &'static str {
        self.image_id
    }

    pub fn repository_env(&self) -> &'static str {
        self.repository_env
    }

    pub fn dockerfile(&self) -> &'static str {
        self.dockerfile
    }

    pub fn context(&self) -> &'static str {
        self.context
    }

    pub fn manifest_key(&self) -> &'static str {
        self.manifest_key
    }

    pub fn artifact_name(&self) -> &'static str {
        self.artifact_name
    }

    pub fn cache_scope(&self) -> &'static str {
        self.cache_scope
    }

    pub fn build_kind(&self) -> &'static str {
        self.build_kind
    }
}

const IMAGE_BUILD_TARGETS: [ImageBuildTargetSpec; 3] = [
    ImageBuildTargetSpec {
        image_id: "runner",
        repository_env: "RUNNER_IMAGE_REPOSITORY",
        dockerfile: "./Dockerfile",
        context: ".",
        manifest_key: "runner_image_ref",
        artifact_name: "published-image-runner",
        cache_scope: "publish-image-runner",
        build_kind: "rust-workspace-musl",
    },
    ImageBuildTargetSpec {
        image_id: "setup-sql",
        repository_env: "SETUP_SQL_IMAGE_REPOSITORY",
        dockerfile: "./crates/setup-sql/Dockerfile",
        context: ".",
        manifest_key: "setup_sql_image_ref",
        artifact_name: "published-image-setup-sql",
        cache_scope: "publish-image-setup-sql",
        build_kind: "rust-workspace-musl",
    },
    ImageBuildTargetSpec {
        image_id: "verify",
        repository_env: "VERIFY_IMAGE_REPOSITORY",
        dockerfile: "./cockroachdb_molt/molt/Dockerfile",
        context: "./cockroachdb_molt/molt",
        manifest_key: "verify_image_ref",
        artifact_name: "published-image-verify",
        cache_scope: "publish-image-verify",
        build_kind: "verify-go",
    },
];

pub struct ImageBuildTargetContract;

impl ImageBuildTargetContract {
    pub fn all() -> &'static [ImageBuildTargetSpec] {
        &IMAGE_BUILD_TARGETS
    }

    pub fn find(image_id: &str) -> &'static ImageBuildTargetSpec {
        Self::all()
            .iter()
            .find(|target| target.image_id() == image_id)
            .unwrap_or_else(|| panic!("unknown image build target `{image_id}`"))
    }
}
