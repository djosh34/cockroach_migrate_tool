# Plan: Prune MOLT Down To The Verify-Only Source Slice

## References

- Task: `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal.md`
- Follow-up task: `.ralph/tasks/story-18-verify-http-image/02-task-upgrade-the-verify-slice-to-go-1-26-and-bump-all-dependencies.md`
- Follow-up task: `.ralph/tasks/story-18-verify-http-image/03-task-build-a-scratch-verify-image-from-the-pruned-verify-source.md`
- Current repo CI and image contract tests:
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
- Current vendored Go entry points:
  - `cockroachdb_molt/molt/main.go`
  - `cockroachdb_molt/molt/cmd/root.go`
  - `cockroachdb_molt/molt/cmd/verify/verify.go`
- Current vendored Go module manifest:
  - `cockroachdb_molt/molt/go.mod`
  - `cockroachdb_molt/molt/go.sum`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- This task is source-pruning and proof-of-removal only.
  - It must not absorb the Go 1.26/dependency-refresh work from task 02.
  - It must not absorb the scratch-image packaging work from task 03.
- The repo's required validation lanes are still Rust-only:
  - `make check`
  - `make lint`
  - `make test`
  - `make test-long`
- The current GitHub workflow installs Rust but not Go, so the guardrails added in this task must be pure repository-contract audits in Rust rather than cargo tests that shell out to `go`.
- Local Go commands are still useful during execution to verify the prune design, but Go must not become a hidden prerequisite for the required cargo validation lanes in this task.
- The verify-only source slice should be pruned in place under `cockroachdb_molt/molt` rather than copied into a second parallel tree.
  - Duplicating the source would create a second drifting package graph and would be a boundary regression.
- Keep the existing top-level `molt` command boundary for now, but reduce it to verify-only wiring.
  - Task 01 should not invent the final container entrypoint or HTTP service contract early.
- If the first RED slice shows that the retained verify packages still require a materially larger cross-cutting support surface than the computed slice below, or that a pure Rust repo-contract audit cannot express the necessary guardrails without becoming brittle, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Identified Verify Build Slice

- The repo-local Go package slice currently needed by `go list -deps ./cmd/verify` is:
  - `cmd/internal/cmdutil`
  - `cmd/verify`
  - `comparectx`
  - `dbconn`
  - `dbtable`
  - `moltlogger`
  - `molttelemetry`
  - `mysqlconv`
  - `mysqlurl`
  - `oracleconv`
  - `parsectx`
  - `pgconv`
  - `retry`
  - `rowiterator`
  - `utils`
  - `utils/typeconv`
  - `verify`
  - `verify/dbverify`
  - `verify/inconsistency`
  - `verify/rowverify`
  - `verify/tableverify`
  - `verify/verifymetrics`
- The actual binary entry surface also still needs:
  - `main.go`
  - `cmd/root.go`
  - `cmd/apiversion.go`
- The current obvious out-of-slice top-level vendored directories are:
  - `.github`
  - `build`
  - `climanifest`
  - `compression`
  - `demo`
  - `docker`
  - `docs`
  - `e2e`
  - `fetch`
  - `moltcsv`
  - `scaletesting`
  - `scripts`
- `testutils` is a special case.
  - It is not required for the runtime build slice.
  - Some retained verify-path Go tests still reference it today.
  - Execution should either delete it after proving nothing retained needs it, or keep it as an explicit test-only exception with a narrow contract instead of letting it remain as an unexamined spillover bucket.

## Interface And Boundary Decisions

- The vendored Go tree becomes a verify-only source boundary.
  - No fetch command.
  - No utility subcommands unrelated to verify.
  - No demo/docs/docker/teamcity baggage inside the retained source tree.
- `cockroachdb_molt/molt/cmd/root.go` should expose only:
  - help/version
  - `verify`
- `cmd/internal/cmdutil` remains the shared verify command utility package, but it must be file-pruned to the pieces verify actually uses.
  - `pprof.go` is fetch-only dead code in the current build path and should be removed.
- Repository-level proof-of-removal belongs in one Rust support boundary.
  - One support module should own the allowlist/forbidden-list for the verify source slice.
  - Do not scatter raw string lists across multiple tests.
- Dependency pruning belongs to the same source-slice boundary.
  - After deleting fetch-only code, `go.mod` and `go.sum` should be reduced to the retained verify slice with the current toolchain, without taking on the Go 1.26/version-bump scope from task 02.

## Improve-Code-Boundaries Focus

- Primary smell: wrong-placeism in the vendored MOLT root.
  - The root `cmd` package currently mixes verify, fetch, and password-escape utility wiring into one public command surface even though this repo's verify image workstream only needs verify.
  - Flatten that boundary so the root Go command is verify-only.
- Secondary smell: fetch-specific behavior still lives inside retained packages.
  - `cmd/internal/cmdutil/pprof.go` is the clearest example.
  - Execution should look for other fetch-only file-level leftovers inside retained packages and delete them instead of leaving dead helpers in the verify build path.
- Test-boundary smell: repo-level source-slice enforcement would be easy to scatter as ad hoc `contains("fetch")` assertions.
  - Centralize those rules in one Rust support module with typed helper methods so the contract is readable and kept in one place.

## Public Contract To Establish

- The vendored Go source under `cockroachdb_molt/molt` is intentionally verify-only.
- The root Go CLI no longer exposes or imports:
  - `fetch`
  - `escape-password`
  - any other non-verify subcommand
- The verify source tree no longer contains clearly unrelated top-level source directories such as fetch, demo, docker, docs, or TeamCity/build assets.
- The retained source tree does not silently carry fetch-only shared files inside otherwise-kept packages.
- Rust repo-contract tests fail loudly if a deleted out-of-slice directory, file, or forbidden dependency drifts back into the verify source tree.
- The required repository cargo lanes remain green without introducing a hidden Go requirement into CI.

## Files And Structure To Add Or Change

- [ ] `crates/runner/tests/ci_contract.rs`
  - add verify-source-slice contract coverage to the existing repo-level contract lane
- [ ] `crates/runner/tests/support/verify_source_contract.rs`
  - new shared support owner for allowed top-level entries, forbidden entries, forbidden root-command markers, and dependency-boundary assertions
- [ ] `cockroachdb_molt/molt/main.go`
  - keep only the minimal verify-root entrypoint if any root command cleanup requires it
- [ ] `cockroachdb_molt/molt/cmd/root.go`
  - reduce the public command set to verify-only wiring
- [ ] `cockroachdb_molt/molt/cmd/escape_password.go`
  - delete as unrelated utility surface if nothing retained still needs it
- [ ] `cockroachdb_molt/molt/cmd/fetch/`
  - delete entirely
- [ ] `cockroachdb_molt/molt/fetch/`
  - delete entirely
- [ ] `cockroachdb_molt/molt/cmd/internal/cmdutil/pprof.go`
  - delete as fetch-only dead code in the retained package
- [ ] `cockroachdb_molt/molt/go.mod`
  - reduce to the retained verify slice dependencies only
- [ ] `cockroachdb_molt/molt/go.sum`
  - reduce accordingly after the source prune
- [ ] `cockroachdb_molt/molt/.github`
- [ ] `cockroachdb_molt/molt/build`
- [ ] `cockroachdb_molt/molt/climanifest`
- [ ] `cockroachdb_molt/molt/compression`
- [ ] `cockroachdb_molt/molt/demo`
- [ ] `cockroachdb_molt/molt/docker`
- [ ] `cockroachdb_molt/molt/docs`
- [ ] `cockroachdb_molt/molt/e2e`
- [ ] `cockroachdb_molt/molt/moltcsv`
- [ ] `cockroachdb_molt/molt/scaletesting`
- [ ] `cockroachdb_molt/molt/scripts`
  - delete each as out-of-slice unless execution proves one is required by retained verify tests
- [ ] `cockroachdb_molt/molt/testutils`
  - either delete, or keep as an explicit test-only exception after proving retained verify-path tests still need it
- [ ] `cockroachdb_molt/molt/utils/formatting.go`
  - prune fetch-only deterministic helpers if execution confirms they are no longer used by the retained slice

## TDD Execution Order

### Slice 1: Tracer Bullet For A Verified Source-Slice Boundary

- [ ] RED: add one failing Rust repo-contract test that asserts the vendored MOLT tree exposes only the expected verify-oriented top-level entries and root-command markers, and that the current fetch/demo/docs clutter is forbidden
- [ ] GREEN: create the shared `verify_source_contract` support module and delete the first obvious out-of-slice directories plus the root `fetch` command wiring until the tracer test passes
- [ ] REFACTOR: keep the allowlist and forbidden-marker ownership in the support module instead of repeating raw string lists in the test body

### Slice 2: Root Command And Retained-Package Prune

- [ ] RED: add one failing contract that asserts `cmd/root.go` exposes only verify/help/version and that fetch-only file-level leftovers inside retained packages are forbidden
- [ ] GREEN: delete `cmd/escape_password.go`, delete `cmd/internal/cmdutil/pprof.go`, and simplify `cmd/root.go` to the minimal verify-only command surface
- [ ] REFACTOR: remove any dead fetch-only helpers in retained packages that are now single-purpose clutter after the prune

### Slice 3: Dependency Boundary Reduction

- [ ] RED: add one failing repo-contract assertion that `go.mod` no longer carries obviously fetch-only dependencies such as cloud-storage and AWS SDK entries after the source prune
- [ ] GREEN: reduce `go.mod` and `go.sum` to the retained verify slice without upgrading toolchain or taking task-02 version bumps
- [ ] REFACTOR: keep dependency-boundary checks focused on forbidden dependency families rather than brittle full-file snapshots that would fight task 02

### Slice 4: Test-Support Exception Audit

- [ ] RED: add one failing contract that forces an explicit decision on `testutils`
- [ ] GREEN: either delete `testutils` after proving nothing retained uses it, or keep it as the one documented test-only exception and remove any other stray test-only support that is not justified by retained verify-path tests
- [ ] REFACTOR: make the support module distinguish runtime entries from explicit test-only exceptions so the slice stays intentional instead of accidental

### Slice 5: Supplemental Local Go Sanity Check

- [ ] RED: run the smallest local Go build/test command that exercises the retained slice and fix the first compile or prune error it reveals
- [ ] GREEN: keep iterating until the retained verify slice can still build locally without reintroducing deleted code
- [ ] REFACTOR: if this exposes a retained package with mixed verify/fetch responsibilities, delete or split the dead file-level leftovers instead of patching around them

### Slice 6: Repository Lanes

- [ ] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [ ] GREEN: continue until every required lane passes cleanly
- [ ] REFACTOR: do one final `improve-code-boundaries` pass to confirm the verify source contract and allowlist rules are not scattered or stringly

## TDD Guardrails For Execution

- Start with a failing Rust contract test. Do not delete code first and retrofit tests afterward.
- Keep the contract tests repo-shaped and public-surface-shaped.
  - They should prove the source boundary from filesystem layout, root command wiring, and manifest shape.
  - They should not require Go to be installed in CI.
- Be aggressive about deletion.
  - If a directory, file, helper, or command is unrelated to verify and not needed by a retained verify-path test, remove it.
- Do not preserve fetch compatibility, demo assets, or utility commands for hypothetical future reuse.
- Do not widen scope into Go 1.26 upgrades, dependency modernization, scratch-image packaging, or HTTP service behavior.
- If a retained package still mixes verify and fetch responsibilities in one directory, prefer deleting the dead file-level pieces over inventing a second compatibility wrapper.

## Boundary Review Checklist

- [ ] No root Go command wiring imports `cmd/fetch`
- [ ] No root Go command wiring exposes `escape-password`
- [ ] No retained package still compiles fetch-only files like `cmd/internal/cmdutil/pprof.go`
- [ ] No top-level vendored source directories remain for fetch/demo/docker/docs/teamcity baggage
- [ ] No fetch-only dependency families remain in `go.mod`
- [ ] No verify-source contract rules are duplicated across several Rust tests
- [ ] Any retained test-only exception such as `testutils` is explicit and justified

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long`
- [ ] One final `improve-code-boundaries` pass after all lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

Plan path: `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal_plans/2026-04-19-verify-source-slice-prune-plan.md`

NOW EXECUTE
