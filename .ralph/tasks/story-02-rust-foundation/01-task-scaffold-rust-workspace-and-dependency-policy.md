## Task: Scaffold the Rust workspace and dependency policy for the runner <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create the Rust workspace and baseline application structure for the destination runner and source bootstrap tooling without introducing throwaway code or library-install-only tasks. The higher order goal is to establish a disciplined foundation that already reflects the selected design and technology rules before feature work begins.

This task must encode the implementation rules already decided in the design files:
- use `sqlx` for PostgreSQL access
- use `thiserror` for application error types
- use established libraries for HTTP, TLS, config, and CLI instead of reinventing them
- do not create separate tasks whose only purpose is "install package X"
- only introduce a library when this story genuinely needs it to establish the workspace contract

In scope:
- create the Cargo workspace and crate layout
- establish lint/test/check commands and repository conventions
- add baseline dependency policy documentation in repo files where appropriate
- create baseline crate modules for:
  - config
  - errors
  - postgres access
  - webhook runtime
  - reconcile runtime
  - source bootstrap CLI

Out of scope:
- real webhook behavior
- real reconcile logic
- real schema comparison

This task must align with the selected design in `designs/crdb-to-postgres-cdc/`.

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers workspace boot, crate wiring, and baseline command contract
- [ ] The workspace uses `thiserror` for application error types
- [ ] The baseline dependency policy records that `sqlx` and established HTTP/TLS/config/CLI libraries are required and must not be reinvented
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

