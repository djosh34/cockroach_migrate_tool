# Plan: Prune MOLT To The PostgreSQL/CockroachDB Verify Hot Path And Add Root Licensing Notices

## References

- Task: `.ralph/tasks/story-18-verify-http-image/10-task-prune-cockroachdb-molt-down-to-the-postgresql-cockroachdb-verify-hot-path-and-add-root-license-notices.md`
- Earlier verify-slice pruning contract:
  - `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal_plans/2026-04-19-verify-source-slice-prune-plan.md`
- Current repo-contract coverage:
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/verify_source_contract.rs`
- Current retained multi-backend boundary:
  - `cockroachdb_molt/molt/dbconn/dbconn.go`
  - `cockroachdb_molt/molt/rowiterator/scan_iterator.go`
  - `cockroachdb_molt/molt/rowiterator/point_lookup_iterator.go`
  - `cockroachdb_molt/molt/verify/tableverify/query.go`
  - `cockroachdb_molt/molt/verify/shard_table.go`
  - `cockroachdb_molt/molt/verify/verify.go`
- Current public verify entry points:
  - `cockroachdb_molt/molt/cmd/root.go`
  - `cockroachdb_molt/molt/cmd/verify/verify.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/verifyservice/config.go`
- Current root-doc and licensing surface:
  - `README.md`
  - `CONTRIBUTING.md`
  - `cockroachdb_molt/molt/LICENSE`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Current State

- The first-stage source prune is already done.
  - `crates/runner/tests/support/verify_source_contract.rs` exists.
  - `cmd/root.go` is already down to `verify` plus `verifyservice`.
- The current retained build graph is still wider than the supported product boundary.
  - `GOTOOLCHAIN=auto go list -deps ./cmd/verifyservice ./cmd/verify` still reaches:
    - `github.com/cockroachdb/molt/mysqlurl`
    - `github.com/cockroachdb/molt/mysqlconv`
    - `github.com/cockroachdb/molt/oracleconv`
    - `github.com/cockroachdb/molt/molttelemetry`
- The public HTTP config already points toward the intended narrower contract.
  - `verifyservice.DatabaseConfig.ConnectionString()` builds PostgreSQL-style URLs.
  - The verify HTTP service only needs PostgreSQL and CockroachDB connectivity over the PG wire protocol.
- The repo root does not yet expose an explicit licensing split for this mixed tree.
  - There is no root `LICENSE`.
  - There is no root `THIRD_PARTY_NOTICES` or equivalent file explaining that `cockroachdb_molt` remains Apache-2.0 while the Rust root is proprietary.

## Planning Assumptions

- This task is the second-stage prune, not a repeat of task 01.
  - Task 01 proved "verify-only versus fetch/demo/docs".
  - Task 10 proves "PostgreSQL/CockroachDB-only versus fake multi-backend support and telemetry".
- The supported runtime contract is:
  - source database: PostgreSQL or CockroachDB
  - destination database: PostgreSQL or CockroachDB
  - transport: PostgreSQL connection URLs and PG-wire clients only
- MySQL and Oracle support should be deleted, not hidden behind dead branches or dormant packages.
- Telemetry/phone-home behavior should be deleted, not stubbed.
  - Local Prometheus metrics used by the verify runtime are still part of the supported hot path and are not considered phone-home telemetry.
- Root licensing clarity is a public repo contract and should be enforced by tests rather than left to convention.
- `make test-long` is not part of the default task gate here.
  - Run `make check`, `make lint`, and `make test`.
  - Only run `make test-long` if execution ends up changing the long-lane selection or the task explicitly proves it is required.
- If the first RED slice shows that the verify engine truly requires a materially different public contract than "PG-wire-only PostgreSQL/CockroachDB verification with no telemetry", this plan is wrong and must be switched back to `TO BE VERIFIED` immediately.

## Improve-Code-Boundaries Focus

- Primary smell: the retained verify slice still carries a fake multi-database abstraction.
  - `dbconn`, `rowiterator`, `tableverify`, and `verify` still branch across PostgreSQL, MySQL, and Oracle.
  - That is the wrong boundary now that only PostgreSQL/CockroachDB verification is supported.
- Intended flattening:
  - Collapse the retained connection/runtime surface onto PG-wire connections.
  - Delete backend-specific conversion and URL packages that only exist for MySQL or Oracle.
  - Keep any necessary PostgreSQL versus CockroachDB behavior as a narrow retained distinction inside the PG-oriented code, not as a generic multi-backend framework.
- Secondary smell: telemetry crosses the wrong boundary.
  - `dbconn.RegisterTelemetry` and `molttelemetry` turn connection setup into phone-home plumbing.
  - That should disappear entirely from the verify hot path.
- Third smell: licensing truth is missing from the repo root.
  - Add one explicit root licensing boundary rather than leaving the split implicit.

## Public Contract To Establish

- The retained vendored Go source is intentionally limited to PostgreSQL/CockroachDB verification.
- Public verify entry points reject non-PostgreSQL URL schemes instead of pretending MySQL or Oracle are still supported.
- The retained Go tree no longer contains:
  - `mysqlconv`
  - `mysqlurl`
  - `oracleconv`
  - `molttelemetry`
  - MySQL/Oracle-only testdata, fixtures, and tests
- The retained verify implementation no longer imports MySQL, Oracle, or telemetry packages.
- Repo-contract tests fail loudly if MySQL, Oracle, or telemetry code re-enters the retained verify slice.
- Repo root licensing files explicitly state:
  - root Rust code is `All Rights Reserved - Joshua Azimullah`
  - `cockroachdb_molt/molt` is governed by Apache-2.0 as documented in `cockroachdb_molt/molt/LICENSE`

## Files And Boundaries To Change

- [ ] `crates/runner/tests/ci_contract.rs`
  - extend the existing repo-contract lane with the new second-stage prune and licensing assertions
- [ ] `crates/runner/tests/support/verify_source_contract.rs`
  - tighten the source-slice contract so the allowlist and forbidden-import checks reflect the PostgreSQL/CockroachDB-only boundary
- [ ] `crates/runner/tests/support/repo_license_contract.rs`
  - add one focused support owner for root `LICENSE` and `THIRD_PARTY_NOTICES` assertions instead of scattering root-doc checks
- [ ] `cockroachdb_molt/molt/dbconn/dbconn.go`
  - remove MySQL/Oracle connection routing and telemetry registration
- [ ] `cockroachdb_molt/molt/dbconn/mysql.go`
- [ ] `cockroachdb_molt/molt/dbconn/oracle.go`
- [ ] `cockroachdb_molt/molt/dbconn/mysql_test.go`
  - delete each as unsupported backend code
- [ ] `cockroachdb_molt/molt/mysqlconv/`
- [ ] `cockroachdb_molt/molt/mysqlurl/`
- [ ] `cockroachdb_molt/molt/oracleconv/`
- [ ] `cockroachdb_molt/molt/molttelemetry/`
  - delete each entirely
- [ ] `cockroachdb_molt/molt/rowiterator/scan_iterator.go`
- [ ] `cockroachdb_molt/molt/rowiterator/row_iterator.go`
- [ ] `cockroachdb_molt/molt/rowiterator/point_lookup_iterator.go`
- [ ] `cockroachdb_molt/molt/verify/tableverify/query.go`
- [ ] `cockroachdb_molt/molt/verify/shard_table.go`
- [ ] `cockroachdb_molt/molt/verify/verify.go`
  - remove MySQL/Oracle/telemetry branches while keeping the PostgreSQL/CockroachDB behavior that the verify runtime still needs
- [ ] `cockroachdb_molt/molt/cmd/internal/cmdutil/dbconn.go`
- [ ] `cockroachdb_molt/molt/verifyservice/config.go`
  - reject non-PostgreSQL schemes at the public CLI/config boundary
- [ ] `cockroachdb_molt/molt/testutils/conn.go`
  - remove MySQL helper state so the retained test harness matches the supported backends
- [ ] `cockroachdb_molt/molt/verify/verify_test.go`
- [ ] `cockroachdb_molt/molt/verify/testdata/datadriven/mysql/`
- [ ] `cockroachdb_molt/molt/rowiterator/testdata/scanquery/mysql.ddt`
- [ ] any remaining `mysql` / `oracle` fixtures and tests surfaced by `rg`
  - delete the unsupported test surface rather than preserving it
- [ ] `cockroachdb_molt/molt/go.mod`
- [ ] `cockroachdb_molt/molt/go.sum`
  - reduce dependencies after the code prune
- [ ] `LICENSE`
- [ ] `THIRD_PARTY_NOTICES`
- [ ] `README.md`
  - add the root licensing split and a precise pointer to `cockroachdb_molt/molt/LICENSE`

## TDD Execution Order

### Slice 1: Tracer Bullet For The Supported Public URL Contract

- [ ] RED: add one failing Go test that proves the public verify config/connection boundary rejects non-PostgreSQL schemes for source and destination URLs
- [ ] GREEN: implement the smallest shared validation needed in `verifyservice/config.go` and the CLI/dbconn loading seam so only PostgreSQL-style URLs are accepted
- [ ] REFACTOR: centralize the scheme validation in one narrow PG-wire boundary instead of duplicating ad hoc `strings.Contains` checks

### Slice 2: Tighten The Repo Source Contract Before Deleting Code

- [ ] RED: extend the Rust repo-contract tests so they fail while `mysqlconv`, `mysqlurl`, `oracleconv`, and `molttelemetry` still exist or are still imported
- [ ] GREEN: update `verify_source_contract.rs` and `ci_contract.rs` to define the second-stage PostgreSQL/CockroachDB-only boundary
- [ ] REFACTOR: keep all filesystem/import allowlists and forbidden markers in one support owner instead of stringly assertions spread across test bodies

### Slice 3: Remove Unsupported Backend Packages

- [ ] RED: run the smallest retained Go test/build command after the contract tightening and fix the first break revealed by deleting one unsupported backend slice
- [ ] GREEN: delete MySQL/Oracle packages, their tests, and their fixtures; simplify `dbconn` so the retained runtime is PG-wire-only
- [ ] REFACTOR: if a retained file still mixes PostgreSQL/CockroachDB logic with deleted-backend logic, split or delete the dead file-level pieces instead of preserving mixed abstractions

### Slice 4: Remove Telemetry From The Verify Hot Path

- [ ] RED: add one failing assertion that the retained source tree no longer imports or contains telemetry code
- [ ] GREEN: delete `molttelemetry`, remove `dbconn.RegisterTelemetry`, and remove telemetry calls from the verify path
- [ ] REFACTOR: keep local operational metrics only where they serve the verify runtime directly, with no phone-home side channel

### Slice 5: Clean Retained Tests And Manifest

- [ ] RED: run the retained Go tests and let the first failing compile/test expose the next backend-specific leftover
- [ ] GREEN: delete MySQL/Oracle-only tests and testdata, update the remaining tests to PostgreSQL/CockroachDB expectations only, and prune `go.mod` / `go.sum`
- [ ] REFACTOR: keep the retained test surface aligned to supported behavior, not to upstream legacy coverage

### Slice 6: Root Licensing Contract

- [ ] RED: add one failing Rust repo-contract test that requires explicit root licensing files and an explicit pointer to the vendored Apache-2.0 component
- [ ] GREEN: add root `LICENSE` and `THIRD_PARTY_NOTICES`, and update the root docs with the licensing split
- [ ] REFACTOR: keep the root licensing checks in one support module so repo policy is obvious and not duplicated

### Slice 7: Full Validation

- [ ] RED: run `make check`, `make lint`, and `make test`, fixing the first failing lane at a time
- [ ] GREEN: continue until all required default lanes pass cleanly
- [ ] REFACTOR: do one last `improve-code-boundaries` pass to ensure the retained verify runtime no longer carries fake multi-backend abstractions or scattered licensing assertions

## TDD Guardrails For Execution

- Start with a failing test for each slice.
  - Do not delete code first and retrofit tests afterward.
- Favor public-interface and repo-contract tests over implementation-coupled tests.
  - public URL/config behavior
  - repo filesystem/import boundary
  - retained build/test behavior
- Be aggressive about deletion.
  - This is greenfield and no backwards compatibility is allowed.
- Do not keep placeholder backend adapters, dormant docs, or no-op telemetry shims.
- Do not swallow errors while simplifying the retained runtime.
  - If execution uncovers an error-swallowing pattern that cannot be fixed within this task, record it as an `add-bug` task instead of ignoring it.
- Do not widen scope into unrelated HTTP API work, image publication work, or long-lane/e2e changes unless execution proves they are directly impacted.

## Boundary Review Checklist

- [ ] No retained source files import `github.com/go-sql-driver/mysql`
- [ ] No retained source files import `github.com/sijms/go-ora/v2`
- [ ] No retained source files import `github.com/cockroachdb/molt/molttelemetry`
- [ ] No retained top-level vendored directories remain for `mysqlconv`, `mysqlurl`, `oracleconv`, or `molttelemetry`
- [ ] No retained verify tests or testdata still claim MySQL or Oracle support
- [ ] No repo-contract assertions for this prune are duplicated across multiple support modules
- [ ] Root licensing files explicitly describe the proprietary root and Apache-2.0 vendored component split

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` only if execution proves this task changed the long-lane selection or the task explicitly requires it
- [ ] One final `improve-code-boundaries` pass after the required lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/10-task-prune-cockroachdb-molt-down-to-the-postgresql-cockroachdb-verify-hot-path-and-add-root-license-notices_plans/2026-04-19-pg-crdb-verify-hot-path-prune-and-root-licensing-plan.md`

NOW EXECUTE
