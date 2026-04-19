#[path = "support/github_workflow_contract.rs"]
mod github_workflow_contract_support;
#[path = "support/published_image_contract.rs"]
mod published_image_contract_support;
#[path = "support/repo_license_contract.rs"]
mod repo_license_contract_support;
#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;
#[path = "support/verify_source_contract.rs"]
mod verify_source_contract_support;

use github_workflow_contract_support::GithubWorkflowContract;
use repo_license_contract_support::RepoLicenseContract;
use runner_docker_contract_support::RunnerDockerContract;
use verify_source_contract_support::VerifySourceContract;

#[test]
fn master_image_workflow_triggers_only_on_pushes_to_master() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_pushes_to_master_only();
}

#[test]
fn master_image_workflow_rejects_outsider_controlled_and_drift_prone_triggers() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_rejects_outsider_controlled_and_drift_prone_triggers();
}

#[test]
fn master_image_workflow_keeps_publish_permissions_and_credentials_out_of_validation() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_keeps_publish_permissions_and_credentials_out_of_validation();
}

#[test]
fn master_image_workflow_explicitly_gates_publish_to_the_trusted_master_push_commit() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_publish_is_explicitly_gated_to_the_trusted_master_push_commit();
}

#[test]
fn ci_publish_safety_model_is_documented_for_reviewers() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_ci_publish_safety_model_is_documented();
}

#[test]
fn master_image_workflow_runs_the_full_repository_validation_suite() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_runs_validation_commands(&["make check", "make test", "make test-long"]);
}

#[test]
fn master_image_workflow_publishes_only_commit_tagged_novice_path_images() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_commit_tagged_ghcr_publish_only();
}

#[test]
fn master_image_workflow_scans_each_release_archive_before_publishing() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_scans_the_release_archive_before_publishing();
}

#[test]
fn master_image_workflow_fails_loudly_and_publishes_a_vulnerability_report() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_release_scan_policy_is_explicit_and_visible();
}

#[test]
fn master_image_workflow_keeps_registry_coordinates_in_one_shared_boundary() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_registry_coordinates_are_isolated();
}

#[test]
fn dockerfile_declares_a_scratch_runtime_image_with_only_the_runner_binary() {
    RunnerDockerContract::assert_dockerfile_uses_a_scratch_runtime_with_only_runner_binary();
}

#[test]
fn vendored_molt_tree_is_pruned_to_the_verify_source_slice() {
    let contract = VerifySourceContract::load();

    contract.assert_top_level_entries_are_within_the_verify_slice();
    contract.assert_root_command_does_not_wire_fetch();
}

#[test]
fn vendored_molt_command_surface_is_verify_only() {
    let contract = VerifySourceContract::load();

    contract.assert_cmd_tree_is_verify_only();
    contract.assert_fetch_only_files_are_absent();
}

#[test]
fn vendored_molt_manifest_excludes_fetch_only_dependency_families() {
    let contract = VerifySourceContract::load();

    contract.assert_fetch_only_dependency_families_are_absent();
}

#[test]
fn vendored_molt_tree_excludes_non_postgres_backends_and_telemetry() {
    let contract = VerifySourceContract::load();

    contract.assert_non_pg_verify_legacy_is_absent();
}

#[test]
fn vendored_molt_manifest_declares_go_1_26() {
    let contract = VerifySourceContract::load();

    contract.assert_module_declares_go_version("1.26");
}

#[test]
fn vendored_molt_retained_source_does_not_import_pkg_errors() {
    let contract = VerifySourceContract::load();

    contract.assert_retained_source_does_not_import("github.com/pkg/errors");
}

#[test]
fn vendored_molt_testutils_boundary_is_an_explicit_verify_test_exception() {
    let contract = VerifySourceContract::load();

    contract.assert_testutils_exception_is_narrow_and_explicit();
}

#[test]
fn repo_root_license_boundary_is_explicit() {
    let contract = RepoLicenseContract::load();

    contract.assert_root_declares_proprietary_rust_workspace_and_apache_vendored_component();
}
