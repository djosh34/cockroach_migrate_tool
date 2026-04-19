# Plan: End-To-End Runtime Shape Integrity For Container, TLS, Cockroach, And Scoped Role

## References

- Task: `.ralph/tasks/story-12-verify-e2e-integrity/02-task-assert-single-container-tls-and-scoped-role-integrity.md`
- Neighboring story-12 tasks that must stay separate:
  - `.ralph/tasks/story-12-verify-e2e-integrity/01-task-assert-e2e-suite-has-no-cheating.md`
  - `.ralph/tasks/story-12-verify-e2e-integrity/03-task-assert-no-post-setup-source-commands-in-e2e.md`
- Existing E2E/runtime support:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/long_lane.rs`
  - `crates/runner/tests/e2e_integrity_contract.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
- Container and runtime contract:
  - `Dockerfile`
  - `README.md`
- Design requirements:
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown plus `designs/crdb-to-postgres-cdc/07_test_strategy.md` are treated as approval for the interface and behavior priorities in this turn.
- This task is about the honesty of the E2E runtime shape, not just the honesty of the code path. The suite must prove it runs in the same operational shape described to users:
  - one destination container
  - HTTPS webhook ingress
  - real CockroachDB
  - scoped PostgreSQL role without superuser
- This task must not absorb neighboring work:
  - task 01 already owns generic "no cheating" integrity guardrails
  - task 03 owns the stricter proof that no extra source-side commands run after CDC setup completes
- If the first RED slice proves the current E2E harness cannot honestly exercise the production-shaped destination runtime without replacing the host-process runner path, execution may remove or demote the host-run E2E path instead of preserving it. No backwards compatibility is required in this repo.

## Problem To Fix

- The current long-lane E2E support proves real webhook, helper, reconcile, and verify behavior, but it still runs the `runner` binary directly on the host in `CdcE2eHarness`.
- That is a boundary smell for this story:
  - `e2e_harness.rs` mixes source/destination Docker orchestration with host-process runtime startup
  - the honest production shape lives partly in `RunnerImageHarness`, partly in `Dockerfile`/`README`, and partly in the host-run E2E harness
  - there is no single typed integrity boundary that says the E2E suite used one destination container, TLS, real CockroachDB, and a non-superuser destination role
- If left as-is, the suite can drift toward a simpler runtime than production while still claiming end-to-end coverage.

## Boundary And Interface Decisions

- Extend the existing typed integrity boundary in `crates/runner/tests/support/e2e_integrity.rs`; do not create a second integrity module.
- Flatten the current wrong place/mixed-responsibility boundary in `crates/runner/tests/support/e2e_harness.rs`:
  - orchestration of Cockroach/Postgres containers should stay in the harness
  - destination runtime mode selection and its observable integrity evidence should become a typed boundary
  - host-runner-only assumptions should not stay scattered across config rendering and test assertions
- Introduce one typed runtime-shape audit exposed through public test support.
  - suggested shape: `RuntimeShapeAudit`
  - responsibilities:
    - assert the destination runtime is started through exactly one runner container for the E2E scenario
    - assert the source bootstrap sink URL is HTTPS and targets the destination runtime shape honestly
    - assert CockroachDB evidence comes from the real Cockroach container/image rather than a fake or local stub
    - assert the destination PostgreSQL role exists, is the configured runtime role, and is not superuser
- Keep customer-specific behavior helpers in `DefaultBootstrapHarness`.
- Keep low-level Docker and SQL mechanics in `CdcE2eHarness` and `RunnerImageHarness`.
- Prefer removing or collapsing the host-run destination path for long-lane E2E if that yields a cleaner honest boundary. This is greenfield code; no compatibility layer is needed just to preserve an easier harness.

## Public Contract To Establish

- The honest E2E path should be:
  - real Cockroach container
  - real PostgreSQL container
  - one destination runner container built from the repo `Dockerfile`
  - HTTPS health and ingest endpoints served by that container
  - source bootstrap rendered against that HTTPS endpoint
  - runtime bootstrap, helper apply, reconcile, and verify all driven through that same destination container contract
- Fast integrity contract tests should fail if:
  - long-lane E2E support regresses back to a host-process destination runtime for the main end-to-end path
  - the source bootstrap webhook base URL for the honest path is non-TLS
  - the honest path stops using the real Cockroach container/image
  - a destination role used by the E2E runtime is or becomes superuser
- The ignored long-lane suite should contain at least one scenario that proves:
  - the destination runtime is the built runner image with entrypoint `runner`
  - HTTPS health succeeds against the running destination container
  - source bootstrap and live update delivery work against that container
  - helper shadow and real destination tables converge while the runtime uses the configured non-superuser destination role

## TDD Approval And Behavior Priorities

- Highest-priority behaviors to prove:
  - the main E2E path runs through one destination container, not a host-only shortcut
  - HTTP ingress for the honest path is HTTPS
  - the honest path uses real CockroachDB
  - the destination runtime succeeds with a scoped PostgreSQL role whose `rolsuper` flag is false
- Lower-priority implementation concerns:
  - preserve the task-01 typed integrity style instead of adding new stringly checks everywhere
  - keep long-lane scenario code readable by pushing runtime-shape evidence behind typed support

## Vertical TDD Slices

### Slice 1: Tracer Bullet For Runtime-Shape Audit

- [x] RED: add one failing fast integrity-contract test that requires a typed runtime-shape audit boundary in `e2e_integrity.rs` instead of ad hoc assertions spread across `default_bootstrap_long_lane.rs`, `e2e_harness.rs`, and `runner_image_harness.rs`
- [x] GREEN: add the minimum typed audit structure and support hooks needed to expose container-count, TLS, Cockroach image, and destination-role evidence through public test support
- [x] REFACTOR: keep runtime-shape parsing and assertions in the integrity boundary, not in scenario files

### Slice 2: Honest Destination Runtime Path

- [x] RED: add one failing fast or ignored-long test that proves the honest E2E path cannot keep using a host-process destination runtime for the main default-bootstrap flow
- [x] GREEN: route the default honest E2E path through the runner image/container path
- [x] REFACTOR: remove or collapse now-redundant host-run E2E runtime setup if it only exists as a simpler shortcut for the same story

### Slice 3: HTTPS Contract

- [x] RED: add a failing assertion that the honest source bootstrap config and runtime shape use `https://...` only
- [x] GREEN: make the honest path expose and consume the TLS endpoint exclusively
- [x] REFACTOR: keep TLS endpoint construction in one place so tests do not duplicate URL-shape logic

### Slice 4: Real CockroachDB Proof

- [x] RED: add a failing integrity assertion that the honest E2E path uses the real Cockroach container/image contract, not a fake process or file fixture
- [x] GREEN: surface the concrete Cockroach runtime evidence already implied by `e2e_harness.rs`
- [x] REFACTOR: centralize that proof in the typed integrity boundary so scenario files do not know about image strings or wrapper internals

### Slice 5: Scoped PostgreSQL Role Proof

- [x] RED: add a failing test that requires explicit audit evidence for the configured destination role and its non-superuser status
- [x] GREEN: expose a typed destination-role audit backed by PostgreSQL metadata queries plus successful runtime bootstrap/apply behavior
- [x] REFACTOR: keep role metadata and scope assertions in support code; long-lane scenarios should read like "runtime uses scoped role", not raw `pg_roles` SQL plumbing

### Slice 6: Single-Container Apply Proof

- [x] RED: strengthen the long-lane scenario so it proves the same destination container that serves HTTPS also performs helper bootstrap and PostgreSQL apply
- [x] GREEN: add only the minimum evidence needed to show helper tables and live updates converge under that one containerized runtime
- [x] REFACTOR: if container-oriented orchestration now belongs in `RunnerImageHarness`, move it there and keep `DefaultBootstrapHarness` customer-focused

### Slice 7: Improve-Code-Boundaries Pass

- [x] RED: if runtime-shape assertions are still split awkwardly across `default_bootstrap_long_lane.rs`, `runner_image_harness.rs`, and `e2e_harness.rs`, add the next failing assertion that exposes the duplication
- [x] GREEN: consolidate the duplication behind the typed integrity boundary and the correct harness owner
- [x] REFACTOR: remove leftover helpers or string checks that no longer have a reason to exist

### Slice 8: Full Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so runtime-shape evidence has one clear owner

## Guardrails For Execution

- Every new test or strengthened assertion must fail before the supporting code is added.
- Do not satisfy this task with README-only or Dockerfile-only string checks. At least one long-lane assertion must use typed runtime-shape evidence from the honest E2E path.
- Do not add a test-only runtime env var, CLI flag, or alternate product path to make container/TLS/role assertions easier.
- Do not preserve a host-run E2E runtime solely for convenience if it conflicts with the honest production shape.
- Do not absorb task 03. If proving these runtime-shape assertions requires auditing post-setup source commands, switch back to `TO BE VERIFIED` and stop.
- Do not swallow Docker, SQL, health-check, TLS, or role-audit failures.

## Boundary Review Checklist

- [x] One typed integrity boundary owns runtime-shape evidence
- [x] The honest long-lane path uses one destination runner container
- [x] HTTPS is asserted through executable behavior, not only docs
- [x] Real CockroachDB usage is asserted explicitly
- [x] Destination-role scope is asserted explicitly and forbids superuser
- [x] No convenience host-runtime shortcut remains in the honest E2E path without a strong reason
- [x] No error path is swallowed or silently ignored

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
