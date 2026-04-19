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
