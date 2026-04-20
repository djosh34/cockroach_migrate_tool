#[path = "support/operator_cli_surface.rs"]
mod operator_cli_surface_support;

use operator_cli_surface_support::OperatorCliSurface;

#[test]
fn operator_cli_surface_lists_every_readme_facing_surface_in_one_place() {
    let surfaces = OperatorCliSurface::documented();
    let surface_ids = surfaces
        .iter()
        .map(OperatorCliSurface::id)
        .collect::<Vec<_>>();

    assert_eq!(
        surface_ids,
        vec!["setup-sql", "runner", "verify-service-image"],
        "operator CLI surface contract must enumerate the README-facing binaries in one shared owner",
    );
}

#[test]
fn operator_cli_surface_encodes_the_depth_policy_for_readme_flows() {
    assert_eq!(
        OperatorCliSurface::setup_sql().allowed_actions(),
        ["emit-cockroach-sql", "emit-postgres-grants"],
        "setup-sql must stay a flat two-action CLI for the README flow",
    );
    assert_eq!(
        OperatorCliSurface::setup_sql().max_action_depth(),
        1,
        "setup-sql must keep one user-visible action level",
    );

    assert_eq!(
        OperatorCliSurface::runner().allowed_actions(),
        ["validate-config", "run"],
        "runner must stay a flat two-action CLI for the README flow",
    );
    assert_eq!(
        OperatorCliSurface::runner().max_action_depth(),
        1,
        "runner must keep one user-visible action level",
    );

    assert_eq!(
        OperatorCliSurface::verify_service_image().allowed_actions(),
        ["run"],
        "the published verify image must stay direct at the user-visible surface",
    );
    assert_eq!(
        OperatorCliSurface::verify_service_image().max_action_depth(),
        0,
        "the published verify image entrypoint must not add another visible action layer",
    );
}
