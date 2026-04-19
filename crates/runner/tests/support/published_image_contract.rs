pub struct PublishedImageContract;

impl PublishedImageContract {
    pub fn registry_host() -> &'static str {
        "ghcr.io"
    }

    pub fn runner_image_repository() -> &'static str {
        "cockroach-migrate-runner"
    }

    pub fn source_bootstrap_image_repository() -> &'static str {
        "cockroach-migrate-source-bootstrap"
    }
}
