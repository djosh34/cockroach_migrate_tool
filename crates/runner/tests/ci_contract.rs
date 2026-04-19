#[path = "support/github_workflow_contract.rs"]
mod github_workflow_contract_support;
#[path = "support/runner_docker_contract.rs"]
mod runner_docker_contract_support;

use github_workflow_contract_support::GithubWorkflowContract;
use runner_docker_contract_support::RunnerDockerContract;

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
fn master_image_workflow_publishes_only_a_commit_tagged_ghcr_image() {
    let workflow = GithubWorkflowContract::load_master_image();

    workflow.assert_commit_tagged_ghcr_publish_only();
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
