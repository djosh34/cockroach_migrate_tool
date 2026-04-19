pub struct PublishedImageSpec {
    image_id: &'static str,
    repository: &'static str,
}

impl PublishedImageSpec {
    const fn new(image_id: &'static str, repository: &'static str) -> Self {
        Self {
            image_id,
            repository,
        }
    }

    pub fn image_id(&self) -> &'static str {
        self.image_id
    }

    pub fn repository(&self) -> &'static str {
        self.repository
    }
}

const PUBLISHED_IMAGES: [PublishedImageSpec; 3] = [
    PublishedImageSpec::new("runner", "cockroach-migrate-runner"),
    PublishedImageSpec::new("setup-sql", "cockroach-migrate-setup-sql"),
    PublishedImageSpec::new("verify", "cockroach-migrate-verify"),
];

pub struct PublishedImageContract;

impl PublishedImageContract {
    pub fn registry_host() -> &'static str {
        "ghcr.io"
    }

    pub fn all() -> &'static [PublishedImageSpec] {
        &PUBLISHED_IMAGES
    }
}
