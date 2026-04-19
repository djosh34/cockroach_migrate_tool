use crate::published_image_contract_support::PublishedImageContract;

pub struct ReadmePublishedImageContract;

impl ReadmePublishedImageContract {
    pub fn assert_setup_sql_quick_start_uses_published_image(setup_sql_quick_start: &str) {
        let setup_sql_image = PublishedImageContract::all()
            .iter()
            .find(|image| image.image_id() == "setup-sql")
            .expect("setup-sql published image should be defined");
        let required_markers = [
            "export GITHUB_OWNER=<github-owner>".to_owned(),
            "export IMAGE_TAG=<published-commit-sha>".to_owned(),
            format!(
                "export SETUP_SQL_IMAGE=\"{}/${{GITHUB_OWNER}}/{}:${{IMAGE_TAG}}\"",
                PublishedImageContract::registry_host(),
                setup_sql_image.repository(),
            ),
            "docker pull \"${SETUP_SQL_IMAGE}\"".to_owned(),
            "\"${SETUP_SQL_IMAGE}\" \\".to_owned(),
            "emit-cockroach-sql \\".to_owned(),
            "--config /config/cockroach-setup.yml > cockroach-bootstrap.sql".to_owned(),
            "emit-postgres-grants \\".to_owned(),
            "--config /config/postgres-grants.yml > postgres-grants.sql".to_owned(),
        ];
        for required_marker in &required_markers {
            assert!(
                setup_sql_quick_start.contains(required_marker),
                "README setup-sql quick start must include `{required_marker}`",
            );
        }
    }

    pub fn assert_runner_quick_start_uses_published_image(docker_quick_start: &str) {
        let runner_image = PublishedImageContract::all()
            .iter()
            .find(|image| image.image_id() == "runner")
            .expect("runner published image should be defined");
        let required_markers = [
            "export GITHUB_OWNER=<github-owner>".to_owned(),
            "export IMAGE_TAG=<published-commit-sha>".to_owned(),
            format!(
                "export RUNNER_IMAGE=\"{}/${{GITHUB_OWNER}}/{}:${{IMAGE_TAG}}\"",
                PublishedImageContract::registry_host(),
                runner_image.repository(),
            ),
            "docker pull \"${RUNNER_IMAGE}\"".to_owned(),
            "\"${RUNNER_IMAGE}\" \\".to_owned(),
            "validate-config --config /config/runner.yml".to_owned(),
            "run --config /config/runner.yml".to_owned(),
        ];
        for required_marker in &required_markers {
            assert!(
                docker_quick_start.contains(required_marker),
                "README Docker quick start must include `{required_marker}`",
            );
        }
    }

    pub fn assert_readme_has_no_wrapper_handoff(docker_quick_start: &str) {
        assert!(
            docker_quick_start.contains("There is no wrapper shell script in the user path."),
            "README Docker quick start must explicitly state that wrapper shell scripts are not part of the public container path",
        );
        for forbidden_marker in ["bash ", ".sh", "/bin/sh", "/bin/bash"] {
            assert!(
                !docker_quick_start.contains(forbidden_marker),
                "README Docker quick start must not hand the operator path off to `{forbidden_marker}`",
            );
        }
    }

    pub fn assert_text_excludes_local_novice_steps(text: &str, context: &str) {
        for forbidden_marker in [
            "cargo run -p setup-sql",
            "cargo install",
            "rustup",
            "docker build -t cockroach-migrate-runner .",
            "docker build ",
            "git clone",
            "clone the repo",
            "clone this repository",
        ] {
            assert!(
                !text.contains(forbidden_marker),
                "{context}: found forbidden novice-path marker `{forbidden_marker}`",
            );
        }
    }
}
