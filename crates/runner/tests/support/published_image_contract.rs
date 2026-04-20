use crate::published_runtime_artifact_contract_support::{
    PublishedRuntimeArtifactContract, PublishedRuntimeArtifactSpec,
};

pub type PublishedImageSpec = PublishedRuntimeArtifactSpec;

pub struct PublishedImageContract;

impl PublishedImageContract {
    pub fn operator_pull_registry_host() -> &'static str {
        "ghcr.io"
    }

    pub fn all() -> &'static [PublishedImageSpec] {
        PublishedRuntimeArtifactContract::all()
    }
}
