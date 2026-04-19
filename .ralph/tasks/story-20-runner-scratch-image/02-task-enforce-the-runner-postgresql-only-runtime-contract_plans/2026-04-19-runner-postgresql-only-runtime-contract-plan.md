# Plan: Enforce The Runner PostgreSQL-Only Runtime Contract

## References

- Task: `.ralph/tasks/story-20-runner-scratch-image/02-task-enforce-the-runner-postgresql-only-runtime-contract.md`
- Related prior plans:
  - `.ralph/tasks/story-16-runtime-split-removals/01-task-remove-runner-source-cockroach-access-and-config_plans/2026-04-19-runner-source-access-removal-plan.md`
  - `.ralph/tasks/story-16-runtime-split-removals/02-task-remove-runner-side-verify-capability-and-code-paths_plans/2026-04-19-runner-verify-removal-plan.md`
  - `.ralph/tasks/story-20-runner-scratch-image/01-task-build-the-runner-as-a-scratch-image-with-one-binary-that-applies-webhook-requests-to-postgresql_plans/2026-04-19-runner-scratch-image-apply-plan.md`
- Current runtime/config surface:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/routing.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
- Current contract and integrity coverage:
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/image_contract.rs`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/e2e_integrity_contract.rs`
  - `crates/runner/tests/support/runner_public_contract.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/e2e_integrity_contract_support.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and behavior priorities in this turn.
- The runner public CLI must remain runtime-only:
  - `validate-config --config <path>`
  - `run --config <path>`
- Source metadata is still allowed as routing metadata:
  - `mappings.source.database`
  - `mappings.source.tables`
  It must not grow back into a source connection contract.
- The runner may accept inbound TLS webhook traffic and may open outbound PostgreSQL connections to configured destination databases. It must not gain any source-database, verify-service, or generic arbitrary-client network path.
- The current code already uses `sqlx::postgres` for outbound database access, but that boundary is implicit and scattered. This task should make it explicit and regression-proof instead of relying on architecture memory.
- If the first RED slice proves that honest runner startup still requires a source connection field, a verify endpoint, or a second outbound network client beyond PostgreSQL destination access, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - runner config allows only destination PostgreSQL connection settings plus source metadata
  - runner runtime owns only two network surfaces:
    - inbound TLS webhook listener
    - outbound PostgreSQL destination connections
  - runner cannot regain Cockroach/source connection settings
  - runner cannot regain verify behavior or verify network targets
  - CI/default tests fail loudly on any boundary regression
- Lower-priority concerns:
  - preserving the current generic `destination.connection` wrapper if a flatter typed PostgreSQL target makes the contract clearer
  - preserving scattered string-list contract checks when one shared boundary helper can own them

## Problem To Fix

- Story-16 removed the obvious runner source/verify CLI and config surface.
- Story-20 task 01 proved the scratch image and direct PostgreSQL apply behavior.
- The remaining gap is contract ownership:
  - `config/parser.rs` still accepts a generic-looking destination wrapper instead of exposing one obvious PostgreSQL-only runtime target type
  - runtime code opens PostgreSQL connections in bootstrap and reconcile, but there is no one canonical contract saying these are the only outbound network targets the runner may ever use
  - public contract tests mostly assert removed words in help text; they do not fully lock down future source/verify/network drift in config and runtime shape
  - integrity checks are split between `RunnerPublicContract` and `E2eIntegrityContractAudit` without one runner-runtime-specific owner for allowed network surfaces

## Interface And Boundary Decisions

- Keep the public CLI unchanged:
  - `runner validate-config --config <path>`
  - `runner run --config <path>`
- Make PostgreSQL destination access a first-class typed boundary.
  - Preferred shape:
    - introduce or rename the destination connection type to something explicit such as `PostgresTarget` / `RunnerPostgresTarget`
    - remove any needless generic wrapper that makes destination access look extensible when it is not
    - keep bootstrap, webhook persistence, reconcile, and tracking writes flowing through that same typed PostgreSQL target
- Keep source config shallow and metadata-only.
  - allowed:
    - source database name
    - selected source tables
  - forbidden:
    - source host
    - source port
    - source url
    - source tls or auth blocks
- Keep verify fully out of the runner.
  - no verify config
  - no verify client URL
  - no verify command path
  - no runtime helper that looks like verify work under a new name
- Add one canonical runner runtime contract support boundary in tests that owns:
  - allowed CLI subcommands
  - forbidden source/verify surface markers
  - allowed network-target concepts
  - forbidden runtime modules or client families where a narrow static audit is still needed

## Improve-Code-Boundaries Focus

- Primary smell: the PostgreSQL-only contract is represented indirectly through a generic config wrapper plus scattered `PgConnection::connect_with(...)` calls.
  - Flatten that into one explicit typed PostgreSQL runtime target used across config, startup planning, webhook persistence, reconcile, and tracking paths.
- Secondary smell: runner boundary enforcement is split across `RunnerPublicContract`, ad hoc config tests, and broader E2E integrity audits.
  - Pull runner-runtime-specific rules into one small support owner instead of scattering negative string checks.
- Be aggressive about deletion.
  - If there is a wrapper/type that only preserves the illusion of future non-Postgres destinations, remove it.
  - If a stale verify/source marker list duplicates another contract helper, consolidate it.

## Public Contract To Establish

- Runner config expresses only:
  - webhook TLS listener settings
  - reconcile interval
  - mapping source metadata
  - destination PostgreSQL target settings
- Runner runtime may only:
  - bind the HTTPS webhook listener
  - connect to configured PostgreSQL destination databases
- Runner runtime may not:
  - accept or derive Cockroach/source connection information
  - call a verify service
  - shell out to verification commands
  - construct generic HTTP/database client targets outside the approved runtime surface
- Boundary regressions fail in the default lane without depending on ultra-long E2E execution.

## Files And Structure To Add Or Change

- [ ] `.ralph/tasks/story-20-runner-scratch-image/02-task-enforce-the-runner-postgresql-only-runtime-contract.md`
  - keep this plan path linked
- [ ] `crates/runner/src/config/mod.rs`
  - replace generic destination wrappering with an explicit PostgreSQL target type if that is the smallest clear contract
- [ ] `crates/runner/src/config/parser.rs`
  - reject source/verify/network drift loudly and validate only the approved PostgreSQL destination shape
- [ ] `crates/runner/src/runtime_plan.rs`
  - flow the explicit PostgreSQL target through startup/runtime planning and remove any redundant generic connection indirection
- [ ] `crates/runner/src/postgres_bootstrap.rs`
  - keep bootstrap connecting only through the explicit PostgreSQL target boundary
- [ ] `crates/runner/src/webhook_runtime/mod.rs`
  - keep the inbound listener explicit and avoid any new outbound client surface
- [ ] `crates/runner/src/webhook_runtime/persistence.rs`
  - keep row-batch writes on the same explicit PostgreSQL target boundary
- [ ] `crates/runner/src/reconcile_runtime/mod.rs`
  - keep reconcile connects on the same explicit PostgreSQL target boundary
- [ ] `crates/runner/tests/support/runner_public_contract.rs`
  - expand or rename into the canonical runner runtime contract owner instead of a removed-surface-only helper
- [ ] `crates/runner/tests/cli_contract.rs`
  - reuse the shared contract owner for allowed commands and forbidden source/verify drift
- [ ] `crates/runner/tests/config_contract.rs`
  - add regression coverage for forbidden source connection fields and verify/network fields
- [ ] `crates/runner/tests/image_contract.rs`
  - keep the built image runtime-only and reuse the shared contract owner
- [ ] `crates/runner/tests/ci_contract.rs`
  - add default-lane regression coverage that the runner runtime contract suite remains present and authoritative
- [ ] `crates/runner/tests/e2e_integrity_contract.rs`
  - touch only if one narrow runner-runtime audit belongs there after moving ownership into the dedicated support boundary

## TDD Execution Order

### Slice 1: Tracer Bullet For The Allowed Runner Network Contract

- [ ] RED: add one failing contract test that names the only approved runner network surfaces and fails because no shared helper owns that contract yet
- [ ] GREEN: add or deepen a shared `RunnerPublicContract`-style support boundary that exposes:
  - allowed CLI subcommands
  - forbidden source/verify markers
  - allowed network surface vocabulary
- [ ] REFACTOR: remove duplicated removed-surface lists from the first touched tests

### Slice 2: Reject Source And Verify Config Drift Loudly

- [ ] RED: add failing config contract coverage for legacy or speculative drift such as:
  - `mappings.source.connection`
  - `mappings.source.url`
  - `verify`
  - `verify_http`
  - destination fields that imply a non-Postgres/generic client path
- [ ] GREEN: tighten parser validation and/or config DTO shapes so those forms fail loudly while valid PostgreSQL destination config still passes
- [ ] REFACTOR: keep approved config shape ownership in one boundary helper or fixture builder instead of repeating ad hoc YAML fragments

### Slice 3: Make PostgreSQL Destination Access The Explicit Runtime Boundary

- [ ] RED: add one failing runtime-shape or compile-time-facing contract that proves bootstrap/reconcile/persistence do not each own their own generic connection concept
- [ ] GREEN: flatten the config/runtime boundary around one explicit PostgreSQL target type and route startup, persistence, reconcile, and tracking writes through it
- [ ] REFACTOR: delete any wrapper or alias that preserves fake destination extensibility or duplicate endpoint-label logic

### Slice 4: Prove The Runner Cannot Regain Source Or Verify Responsibilities

- [ ] RED: add one failing default-lane contract that proves the runner runtime surface cannot regain:
  - Cockroach/source connection settings
  - verify commands or verify endpoints
  - extra outbound client families beyond PostgreSQL destination access
- [ ] GREEN: tighten the shared runner runtime contract support and any narrow static audits needed to make this explicit and loud
- [ ] REFACTOR: keep these audits narrow and owned by one support module instead of spreading raw file-content checks across unrelated tests

### Slice 5: Repository Lanes

- [ ] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [ ] GREEN: continue until every required default lane passes cleanly
- [ ] REFACTOR: do one final `improve-code-boundaries` pass over runner config/runtime contract ownership and remove any leftover generic destination or duplicated audit shape

## TDD Guardrails For Execution

- Start each slice with a failing test. Do not refactor first.
- Prefer contract tests through public config parsing, public CLI/help, built-image help, and narrow shared support audits over broad repo greps.
- Do not add compatibility shims for source connection fields, verify endpoints, or generic destination clients. This repo is greenfield and forbids backwards compatibility.
- Do not satisfy the task with a comment or naming-only change while leaving multiple implicit connection concepts in place.
- Do not move verification work back into the runner under a renamed helper.
- Do not run `make test-long` unless execution changes the ultra-long lane or the task later proves it is explicitly required.

## Boundary Review Checklist

- [ ] Runner config accepts only source metadata and destination PostgreSQL target settings
- [ ] No runner config accepts source host/port/url/auth or verify endpoint/config
- [ ] Bootstrap connects only through the explicit PostgreSQL target boundary
- [ ] Webhook persistence connects only through the explicit PostgreSQL target boundary
- [ ] Reconcile connects only through the explicit PostgreSQL target boundary
- [ ] Runner help and image help expose only `validate-config` and `run`
- [ ] Runner tests fail loudly if source/verify/network drift is reintroduced
- [ ] No fake generic destination wrapper survives where a typed PostgreSQL target would do

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` only if execution changes the ultra-long lane or the task proves it is required
- [ ] One final `improve-code-boundaries` pass after all required lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-20-runner-scratch-image/02-task-enforce-the-runner-postgresql-only-runtime-contract_plans/2026-04-19-runner-postgresql-only-runtime-contract-plan.md`

NOW EXECUTE
