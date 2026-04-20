use crate::published_runtime_artifact_contract_support::{
    PublishedRuntimeArtifactContract, PublishedRuntimeArtifactSpec,
};

pub type PublishedImageSpec = PublishedRuntimeArtifactSpec;

pub struct PublishedImageContract;

impl PublishedImageContract {
    pub fn registry_host() -> &'static str {
        PublishedRuntimeArtifactContract::registry_host()
    }

    pub fn all() -> &'static [PublishedImageSpec] {
        PublishedRuntimeArtifactContract::all()
    }
}
