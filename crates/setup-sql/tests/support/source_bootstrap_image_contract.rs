use std::{ffi::OsString, path::PathBuf};

pub fn assert_image_entrypoint_is_direct_setup_sql(image_entrypoint_json: &str) {
    assert_eq!(
        image_entrypoint_json.trim(),
        "[\"/usr/local/bin/setup-sql\"]",
        "setup-sql image must invoke the binary directly instead of using a shell wrapper",
    );
}

pub fn docker_build_image_args(image_tag: &str) -> Vec<OsString> {
    vec![
        OsString::from("build"),
        OsString::from("-t"),
        OsString::from(image_tag),
        OsString::from("-f"),
        source_bootstrap_slice_root()
            .join("Dockerfile")
            .into_os_string(),
        repo_root().into_os_string(),
    ]
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn source_bootstrap_slice_root() -> PathBuf {
    repo_root().join("crates/setup-sql")
}
