pub struct RunnerPublicContract;

#[allow(dead_code)]
impl RunnerPublicContract {
    pub fn documented_subcommands() -> &'static [&'static str] {
        &["validate-config", "run"]
    }

    fn forbidden_removed_surface_markers() -> &'static [&'static str] {
        &[
            "compare-schema",
            "render-helper-plan",
            "render-postgres-setup",
            "verify",
            "cutover-readiness",
            "--source-url",
            "--cockroach-schema",
            "--allow-tls-mode-disable",
        ]
    }

    pub fn assert_text_excludes_removed_surface(text: &str, context: &str) {
        for marker in Self::forbidden_removed_surface_markers() {
            assert!(
                !text.contains(marker),
                "{context}: found removed runner surface marker `{marker}`",
            );
        }
    }
}
