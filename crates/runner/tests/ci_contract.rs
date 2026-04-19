#[path = "support/github_workflow_contract.rs"]
mod github_workflow_contract_support;
#[path = "support/published_image_contract.rs"]
mod published_image_contract_support;
#[path = "support/repo_license_contract.rs"]
mod repo_license_contract_support;
#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;
#[path = "support/runner_public_contract.rs"]
mod runner_public_contract_support;
#[path = "support/verify_source_contract.rs"]
mod verify_source_contract_support;

use github_workflow_contract_support::GithubWorkflowContract;
use repo_license_contract_support::RepoLicenseContract;
use runner_docker_contract_support::RunnerDockerContract;
use runner_public_contract_support::RunnerPublicContract;
use verify_source_contract_support::VerifySourceContract;

#[test]
fn publish_images_workflow_triggers_only_on_pushes_to_main() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_pushes_to_main_only();
}

#[test]
fn publish_images_workflow_rejects_outsider_controlled_and_drift_prone_triggers() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_rejects_outsider_controlled_and_drift_prone_triggers();
}

#[test]
fn publish_images_workflow_keeps_publish_permissions_and_credentials_out_of_validation() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_keeps_publish_permissions_and_credentials_out_of_validation();
}

#[test]
fn publish_images_workflow_explicitly_gates_publish_to_the_trusted_main_push_commit() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_publish_is_explicitly_gated_to_the_trusted_main_push_commit();
}

#[test]
fn ci_publish_safety_model_is_documented_for_reviewers() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_ci_publish_safety_model_is_documented();
}

#[test]
fn publish_images_workflow_runs_required_repository_validation_before_publishing() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_runs_validation_commands(&["make check", "make test"]);
}

#[test]
fn publish_images_workflow_cancels_older_main_runs_when_new_pushes_arrive() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_cancels_older_main_runs_when_new_pushes_arrive();
}

#[test]
fn publish_images_workflow_publishes_the_canonical_three_image_set() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_publishes_the_canonical_three_image_set();
}

#[test]
fn publish_images_workflow_uses_multi_arch_commit_sha_tags_only() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_uses_multi_arch_commit_sha_tags_only();
}

#[test]
fn publish_images_workflow_installs_publish_dependencies_via_direct_shell_steps() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_installs_publish_dependencies_via_direct_shell_steps();
}

#[test]
fn publish_images_workflow_emits_published_image_manifest_for_downstream_consumers() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_emits_published_image_manifest_for_downstream_consumers();
}

#[test]
fn publish_images_workflow_masks_derived_sensitive_values_and_never_logs_raw_credentials() {
    let workflow = GithubWorkflowContract::load_publish_images();

    workflow.assert_masks_derived_sensitive_values_and_never_logs_raw_credentials();
}

#[test]
fn dockerfile_declares_a_scratch_runtime_image_with_only_the_runner_binary() {
    RunnerDockerContract::assert_dockerfile_uses_a_scratch_runtime_with_only_runner_binary();
}

#[test]
fn runner_runtime_contract_explicitly_limits_network_surfaces() {
    RunnerPublicContract::assert_runtime_network_surface_contract();
}

#[test]
fn runner_config_contract_excludes_source_connection_and_verify_drift() {
    RunnerPublicContract::assert_config_surface_contract();
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
