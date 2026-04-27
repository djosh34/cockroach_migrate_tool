use std::collections::BTreeSet;

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
