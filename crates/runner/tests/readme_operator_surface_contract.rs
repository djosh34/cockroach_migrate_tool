#[path = "support/readme_operator_surface.rs"]
mod readme_operator_surface_support;

use readme_operator_surface_support::ReadmeOperatorSurface;

#[test]
fn readme_stays_short_and_operator_only() {
    let readme = ReadmeOperatorSurface::load();

    assert_eq!(
        readme.second_level_headings(),
        vec![
            "## Setup SQL Quick Start",
            "## Runner Quick Start",
            "## Verify Quick Start",
        ],
        "README should stay focused on the three supported operator image flows",
    );

    for forbidden in [
        "git clone",
        "docker build",
        "cargo ",
        "cargo\n",
        "make ",
        "make\n",
        "AGENTS.md",
        "CONTRIBUTING.md",
        "## Licensing",
        "## CI Publish Safety",
        "All Rights Reserved",
        "workflow",
        "pull_request_target",
        "release events",
    ] {
        assert!(
            !readme.text().contains(forbidden),
            "README must stay operator-only and exclude `{forbidden}`",
        );
    }

    assert!(
        readme.word_count() <= 1250,
        "README should stay short enough to behave like a quick-start guide; found {} words",
        readme.word_count(),
    );
}

#[test]
fn readme_starts_with_setup_sql_before_runner_and_verify() {
    let readme = ReadmeOperatorSurface::load();
    let text = readme.text();

    let cockroach_config = text
        .find("Example Cockroach setup config:")
        .expect("README should introduce the setup flow with the Cockroach config example");
    let cockroach_command = text
        .find("Render the Cockroach bootstrap SQL:")
        .expect("README should show how to emit Cockroach bootstrap SQL");
    let postgres_config = text
        .find("Example PostgreSQL grants config:")
        .expect("README should include the PostgreSQL grants config before the runtime sections");
    let runner_heading = text
        .find("## Runner Quick Start")
        .expect("README should include the runner quick start heading");
    let verify_heading = text
        .find("## Verify Quick Start")
        .expect("README should include the verify quick start heading");

    assert!(
        cockroach_config < cockroach_command && cockroach_command < postgres_config,
        "README should start with the simplest setup-sql flow before it adds the grants step",
    );
    assert!(
        postgres_config < runner_heading && runner_heading < verify_heading,
        "README should finish setup-sql guidance before it introduces runner and verify surfaces",
    );
}

#[test]
fn readme_keeps_required_and_optional_args_as_short_lists() {
    let readme = ReadmeOperatorSurface::load();
    let setup_sql = readme.section("## Setup SQL Quick Start");
    let runner = readme.section("## Runner Quick Start");
    let verify = readme.section("## Verify Quick Start");

    for section in [setup_sql, runner, verify] {
        assert!(
            section.contains("Required args:\n\n- `"),
            "README sections should explain required args as short bullet lists",
        );
        assert!(
            section.contains("Optional args:\n\n- `"),
            "README sections should explain optional args as short bullet lists",
        );
    }

    for required_snippet in [
        "- `emit-cockroach-sql`",
        "- `emit-postgres-grants`",
        "- `validate-config --config /config/runner.yml`",
        "- `run --config /config/runner.yml`",
        "- `--config /config/verify-service.yml`",
        "- `--log-format json` for structured stderr logs",
    ] {
        assert!(
            readme.text().contains(required_snippet),
            "README should keep operator args inline; missing `{required_snippet}`",
        );
    }
}

#[test]
fn readme_keeps_inline_operator_files_copyable() {
    let readme = ReadmeOperatorSurface::load();

    for relative_path in [
        "config/cockroach-setup.yml",
        "config/postgres-grants.yml",
        "config/runner.yml",
        "config/verify-service.yml",
        "setup-sql.compose.yml",
        "runner.compose.yml",
        "verify.compose.yml",
    ] {
        let contents = readme.operator_file(relative_path);
        assert!(
            !contents.trim().is_empty(),
            "README should keep `{relative_path}` inline and copyable",
        );
    }

    for (relative_path, required_snippet) in [
        (
            "config/runner.yml",
            "url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a",
        ),
        (
            "config/verify-service.yml",
            "client_ca_path: /config/certs/client-ca.crt",
        ),
        ("setup-sql.compose.yml", "image: \"${SETUP_SQL_IMAGE}\""),
        ("runner.compose.yml", "image: \"${RUNNER_IMAGE}\""),
        ("verify.compose.yml", "image: \"${VERIFY_IMAGE}\""),
    ] {
        assert!(
            readme
                .operator_file(relative_path)
                .contains(required_snippet),
            "README inline operator file `{relative_path}` should contain `{required_snippet}`",
        );
    }

    let verify_config = readme.operator_file("config/verify-service.yml");
    for forbidden_snippet in [
        "transport:",
        "client_auth:",
        "mode: verify-full",
        "mode: verify-ca",
    ] {
        assert!(
            !verify_config.contains(forbidden_snippet),
            "README verify-service config should remove obsolete nested TLS knobs like `{forbidden_snippet}`",
        );
    }
}

#[test]
fn readme_runner_quick_start_recommends_destination_urls_and_keeps_the_explicit_alternative() {
    let readme = ReadmeOperatorSurface::load();
    let runner = readme.section("## Runner Quick Start");
    let runner_config = readme.operator_file("config/runner.yml");

    assert!(
        runner_config.contains(
            "url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a",
        ),
        "README runner config should recommend the concise destination.url shape",
    );
    assert!(
        runner.contains("sslmode=verify-ca")
            && runner.contains("sslrootcert=/config/certs/destination-ca.crt")
            && runner.contains("sslcert=/config/certs/destination-client.crt")
            && runner.contains("sslkey=/config/certs/destination-client.key"),
        "README runner quick start should document the supported TLS query parameters for destination.url",
    );
    assert!(
        runner.contains("Explicit-field alternative:"),
        "README runner quick start should keep the decomposed destination form as an explicit alternative",
    );
    assert!(
        runner.contains("host: pg-a.example.internal")
            && runner.contains("client_key_path: /config/certs/destination-client.key"),
        "README runner quick start should still show the explicit alternative with TLS material",
    );
}

#[test]
fn readme_operator_surface_materializes_the_inline_operator_workspace() {
    let readme = ReadmeOperatorSurface::load();
    let workspace =
        tempfile::tempdir().expect("readme operator-surface temp dir should be created");

    readme.materialize_operator_workspace(workspace.path());

    for relative_path in [
        "config/cockroach-setup.yml",
        "config/postgres-grants.yml",
        "config/runner.yml",
        "config/verify-service.yml",
        "setup-sql.compose.yml",
        "runner.compose.yml",
        "verify.compose.yml",
    ] {
        let path = workspace.path().join(relative_path);
        assert!(
            path.is_file(),
            "README operator surface should materialize `{relative_path}` into the copied workspace",
        );
    }
}

#[test]
fn readme_verify_quick_start_documents_the_http_job_flow() {
    let readme = ReadmeOperatorSurface::load();
    let verify = readme.section("## Verify Quick Start");

    for required_snippet in [
        "\"include_schema\":\"^public$\"",
        "\"include_table\":\"^(accounts|orders)$\"",
        "POST /jobs",
        "GET /jobs/${JOB_ID}",
        "POST /jobs/${JOB_ID}/stop",
        "\"status\":\"running\"",
        "\"status\":\"succeeded\"",
        "\"status\":\"failed\"",
        "\"status\":\"stopping\"",
        "\"category\":\"request_validation\"",
        "\"code\":\"unknown_field\"",
        "\"category\":\"source_access\"",
        "\"code\":\"connection_failed\"",
        "\"category\":\"mismatch\"",
        "\"code\":\"mismatch_detected\"",
    ] {
        assert!(
            verify.contains(required_snippet),
            "README verify quick start should document `{required_snippet}`",
        );
    }
}
