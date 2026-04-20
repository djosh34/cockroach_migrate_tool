#[path = "support/verify_source_contract.rs"]
mod verify_source_contract_support;

use verify_source_contract_support::VerifySourceContract;

#[test]
fn verify_source_stays_scoped_to_the_verify_only_slice() {
    let contract = VerifySourceContract::load();

    contract.assert_top_level_entries_are_within_the_verify_slice();
    contract.assert_cmd_tree_is_verify_only();
    contract.assert_fetch_only_files_are_absent();
    contract.assert_testutils_exception_is_narrow_and_explicit();
    contract.assert_module_declares_go_version("1.26");
}

#[test]
fn verify_source_excludes_fetch_only_and_non_postgres_legacy_dependencies() {
    let contract = VerifySourceContract::load();

    contract.assert_fetch_only_dependency_families_are_absent();
    contract.assert_non_pg_verify_legacy_is_absent();
}

#[test]
fn verify_root_command_exposes_only_verify_surfaces() {
    let contract = VerifySourceContract::load();

    contract.assert_root_command_does_not_wire_fetch();
}
