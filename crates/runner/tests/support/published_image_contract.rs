pub struct PublishedImageContract;

impl PublishedImageContract {
    pub fn registry_host() -> &'static str {
        "ghcr.io"
    }

    pub fn runner_image_repository() -> &'static str {
        "cockroach-migrate-runner"
    }

    pub fn setup_sql_image_repository() -> &'static str {
        "cockroach-migrate-setup-sql"
    }
}
