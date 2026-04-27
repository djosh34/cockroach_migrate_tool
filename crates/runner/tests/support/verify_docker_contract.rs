use std::{collections::BTreeSet, path::PathBuf};

pub fn assert_image_entrypoint_is_direct_verify_surface(image_entrypoint_json: &str) {
    assert_eq!(
        image_entrypoint_json.trim(),
        "[\"/usr/local/bin/molt\",\"verify-service\"]",
        "verify image must expose the verify-service command root directly from the entrypoint",
    );
}

pub fn assert_runtime_filesystem_is_minimal(exported_paths: &[String]) {
    let actual_paths = exported_paths.iter().cloned().collect::<BTreeSet<_>>();
    let expected_paths = BTreeSet::from([
        String::from(".dockerenv"),
        String::from("dev/"),
        String::from("dev/console"),
        String::from("dev/pts/"),
        String::from("dev/shm/"),
        String::from("etc/"),
        String::from("etc/hostname"),
        String::from("etc/hosts"),
        String::from("etc/mtab"),
        String::from("etc/resolv.conf"),
        String::from("proc/"),
        String::from("sys/"),
        String::from("usr/"),
        String::from("usr/local/"),
        String::from("usr/local/bin/"),
        String::from("usr/local/bin/molt"),
    ]);

    assert_eq!(
        actual_paths, expected_paths,
        "verify image runtime filesystem must stay minimal and carry only the verify binary payload",
    );
}

pub fn docker_build_image_args(image_tag: &str) -> Vec<String> {
    vec![
        String::from("build"),
        String::from("-t"),
        image_tag.to_owned(),
        String::from("-f"),
        verify_slice_root().join("Dockerfile").display().to_string(),
        verify_slice_root().display().to_string(),
    ]
}

fn verify_slice_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("cockroachdb_molt/molt")
}
