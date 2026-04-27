use std::{
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use crate::nix_image_artifact_harness_support::NixImageArtifact;

pub(crate) fn runner_image_ref() -> &'static str {
    static RUNNER_IMAGE_REF: OnceLock<String> = OnceLock::new();

    RUNNER_IMAGE_REF.get_or_init(|| {
        let image_ref = format!("cockroach-migrate-runner-novice-{}", unique_suffix());
        NixImageArtifact::new("runner-image", "cockroach-migrate-runner:nix")
            .provision_image_tag(&image_ref, "runner novice image");
        image_ref
    })
}

pub(crate) fn verify_image_ref() -> &'static str {
    static VERIFY_IMAGE_REF: OnceLock<String> = OnceLock::new();

    VERIFY_IMAGE_REF.get_or_init(|| {
        let image_ref = format!("cockroach-migrate-verify-novice-{}", unique_suffix());
        NixImageArtifact::new("verify-image", "cockroach-migrate-verify:nix")
            .provision_image_tag(&image_ref, "verify novice image");
        image_ref
    })
}

fn unique_suffix() -> String {
    static UNIQUE_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos(),
        UNIQUE_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed),
    )
}
