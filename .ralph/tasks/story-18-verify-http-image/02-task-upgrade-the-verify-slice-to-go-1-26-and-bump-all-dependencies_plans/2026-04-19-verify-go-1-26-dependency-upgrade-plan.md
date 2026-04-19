# Plan: Upgrade The Verify Slice To Go 1.26 And Refresh Its Dependencies

## References

- Task: `.ralph/tasks/story-18-verify-http-image/02-task-upgrade-the-verify-slice-to-go-1-26-and-bump-all-dependencies.md`
- Prior task and established source-boundary contract:
  - `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal.md`
  - `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal_plans/2026-04-19-verify-source-slice-prune-plan.md`
- Existing repo-contract entrypoint:
  - `crates/runner/tests/ci_contract.rs`
- Existing verify-slice support boundary:
  - `crates/runner/tests/support/verify_source_contract.rs`
- Current verify-slice module manifests:
  - `cockroachdb_molt/molt/go.mod`
  - `cockroachdb_molt/molt/go.sum`
- Current verify CLI and concrete dependency-boundary smell:
  - `cockroachdb_molt/molt/cmd/verify/verify.go`
  - `cockroachdb_molt/molt/mysqlconv/mysqlconv.go`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- Task 01 already established the filesystem and command-surface boundary for the verify-only vendored tree.
  - Task 02 should extend that same contract boundary instead of creating a second parallel audit surface.
- The required repository validation lanes remain Rust-driven:
  - `make check`
  - `make lint`
  - `make test`
  - `make test-long`
- Those lanes do not run Go directly today.
  - Execution must still run local Go commands to prove the verify slice builds and tests on Go 1.26, but repo-contract tests should stay Rust-based so CI does not silently depend on hidden shell setup.
- This task is the toolchain and dependency-refresh layer only.
  - It must not widen into scratch-image packaging from task 03.
  - It must not widen into the HTTP service/config/API tasks.
- Dependency refresh means modernizing the dependencies actually used by the retained verify slice and deleting dead/deprecated dependencies that no longer belong.
  - It does not mean policing every transitive edge or every incidental checksum line in `go.sum`.
- If the first red slice shows that Go 1.26 forces a materially different public contract than “verify slice builds and tests cleanly with a refreshed manifest”, or that the manifest cannot be modernized without reopening the source-prune boundary from task 01, this plan must stay `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- `cockroachdb_molt/molt/go.mod` in the current dirty workspace already declares `go 1.26`.
- `cockroachdb_molt/molt/mysqlconv/mysqlconv.go` in the current dirty workspace already imports `github.com/cockroachdb/errors` instead of `github.com/pkg/errors`.
- The current Rust contract is what still encodes the wrong boundary:
  - `assert_fetch_only_dependency_families_are_absent` still checks raw `go.sum` text for dependency-family absence
  - `assert_manifest_dependency_is_absent("github.com/pkg/errors")` still requires the module to vanish from `go.mod` and `go.sum`
- `go mod tidy` and `go mod why -m` already demonstrated why that contract is wrong:
  - `github.com/pkg/errors` can remain as an indirect transitive requirement of retained verify-slice dependencies
  - fetch-era modules can leave checksum lines in `go.sum` even when they are not direct requirements of the retained slice
- The retained direct dependency surface still includes runtime and test dependencies such as:
  - `github.com/spf13/cobra`
  - `github.com/prometheus/client_golang`
  - `github.com/jackc/pgx/v5`
  - `github.com/go-sql-driver/mysql`
  - `github.com/sijms/go-ora/v2`
  - `golang.org/x/sync`
  - `golang.org/x/time`
  - `github.com/stretchr/testify`
  - `github.com/cockroachdb/datadriven`

## Interface And Boundary Decisions

- Keep one repository-owned contract boundary for the verify slice:
  - `VerifySourceContract` remains the single owner for manifest/toolchain assertions that are visible to the Rust validation lanes.
- Toolchain contract should be explicit but not brittle:
  - assert the module declares Go 1.26
  - do not snapshot the whole `go.mod`
  - do not assert incidental `go.sum` line absence
- Dependency-boundary assertions should target boundaries we actually own:
  - retained source imports under `cockroachdb_molt/molt`
  - direct requirements in `cockroachdb_molt/molt/go.mod`
  - curated intentional dependency versions in `go.mod`
- `go.sum` is evidence, not contract.
  - Execution may refresh it with `go mod tidy`
  - tests should not fail just because a transitive checksum line exists
- Eliminate duplicate boundary libraries where practical during the refresh.
  - The verify slice should standardize on `github.com/cockroachdb/errors` in retained source code rather than carrying a second explicit `github.com/pkg/errors` import boundary.
- Do not add compatibility wrappers or alternate module files.
  - The verify slice should keep one `go.mod` / `go.sum` pair under `cockroachdb_molt/molt`.

## Improve-Code-Boundaries Focus

- Primary smell: the repo contract currently treats `go.sum` as a first-class architecture boundary.
  - That is the wrong layer.
  - The contract should instead parse and own the direct manifest surface and retained source-import surface.
- Secondary smell: duplicate error abstraction at the package boundary.
  - `mysqlconv/mysqlconv.go` was the only retained explicit `pkg/errors` import.
  - Execution should preserve the switch to `github.com/cockroachdb/errors` and prove that retained source code no longer imports `pkg/errors`.
- Tertiary smell: task 01 and the aborted task 02 work both left stringly manifest checks in `VerifySourceContract`.
  - Execution should deepen that module so direct-requirement parsing and retained-import scanning live there instead of sprinkling raw `contains(...)` checks through test bodies.

## Public Contract To Establish

- The verify-only Go module explicitly targets Go 1.26.
- The verify slice builds and tests locally on Go 1.26 without reintroducing deleted fetch-era code.
- The verify source tree does not import fetch-only dependency families.
- The verify source tree does not import `github.com/pkg/errors`.
- `cockroachdb_molt/molt/go.mod` direct requirements do not reintroduce fetch-only families as retained direct dependencies.
- `cockroachdb_molt/molt/go.mod` direct requirements are refreshed for the retained verify slice according to the intentional version choices made during execution.
- Rust repo-contract tests fail loudly if the module drifts back below Go 1.26, if retained source imports regress, or if direct requirements drift away from the curated verify-slice boundary.

## Files And Structure To Add Or Change

- [x] `crates/runner/tests/ci_contract.rs`
  - replace the invalid full-manifest absence assertions with direct-boundary tests owned by `VerifySourceContract`
- [x] `crates/runner/tests/support/verify_source_contract.rs`
  - add shared helpers for:
    - Go version assertions
    - direct requirement parsing from `go.mod`
    - retained-source import scanning
    - curated dependency/version assertions specific to the verify slice modernization
- [x] `cockroachdb_molt/molt/go.mod`
  - keep Go 1.26 and refresh direct dependency versions used by the retained verify slice
- [x] `cockroachdb_molt/molt/go.sum`
  - refresh the transitive graph after the dependency/toolchain upgrade
- [x] `cockroachdb_molt/molt/mysqlconv/mysqlconv.go`
  - keep the explicit source-level error boundary on `github.com/cockroachdb/errors`
- [x] `cockroachdb_molt/molt/...`
  - any additional compile-fix sites revealed by Go 1.26 or dependency API changes, kept strictly within the retained verify slice

## TDD Execution Order

### Slice 1: Recast The Contract Onto Boundaries We Own

- [x] RED: replace one invalid Rust repo-contract assertion with one failing boundary-oriented assertion in `VerifySourceContract`
  - start with the smallest correction that proves intent, such as “retained verify source files do not import `github.com/pkg/errors`”
  - or “fetch-only dependency families are absent from direct requirements and retained source imports”
- [x] GREEN: implement the corresponding `VerifySourceContract` helper until that new test passes against the current workspace
- [x] REFACTOR: remove or rewrite the invalid `go.sum`/full-manifest absence helpers so `VerifySourceContract` owns one coherent set of boundary rules

### Slice 2: Lock The Explicit Toolchain Contract

- [x] RED: run the focused Rust contract asserting that the verify module declares Go 1.26
- [x] GREEN: preserve or adjust `cockroachdb_molt/molt/go.mod` until the test passes with the corrected contract layout
- [x] REFACTOR: keep Go manifest parsing and failure messages inside `verify_source_contract.rs`

### Slice 3: Duplicate Error Boundary Cleanup

- [x] RED: add one failing contract assertion that retained verify-source files no longer import `github.com/pkg/errors`
- [x] GREEN: preserve the `mysqlconv` switch to `github.com/cockroachdb/errors` and fix any other retained-source import site if one appears
- [x] REFACTOR: if execution exposes any other duplicate boundary libraries in retained source code, collapse them instead of layering compatibility imports on top

### Slice 4: Direct Dependency Refresh

- [x] RED: add one failing contract assertion for the intentional direct-dependency modernization surface
  - keep this scoped to a small curated set of direct requirements the retained slice actually uses
  - prefer direct-requirement assertions in `go.mod` over any `go.sum` snapshot
- [x] GREEN: bump the retained direct dependencies in `go.mod`, run module refresh commands, and fix the first compile/test failure only
- [x] REFACTOR: let `go mod tidy` delete stale direct or transitive baggage; do not preserve muddy manifest entries for removed code

### Slice 5: Local Go Validation On The Retained Slice

- [x] RED: run the smallest end-to-end Go validation command that should fail first under the upgraded toolchain
  - start with a focused verify-slice command such as package tests or build for the vendored module
- [x] GREEN: fix the first failure, then repeat until the verify slice builds and tests cleanly on Go 1.26
- [x] REFACTOR: if Go 1.26 or dependency bumps reveal code living behind the wrong boundary, simplify that boundary instead of layering shims on top

### Slice 6: Repository Validation Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm the manifest assertions and dependency cleanup still live in one coherent contract boundary

## TDD Guardrails For Execution

- Start with a failing contract test before changing the relevant boundary.
- Keep tests behavior-oriented and boundary-oriented.
  - Rust-side tests should prove the repository contract.
  - Local Go commands should prove the upgraded module still behaves.
- Do not “upgrade everything” horizontally.
  - move one tracer bullet at a time
  - fix the first failure at a time
- Do not use `go.sum` as a proxy for architecture intent.
  - transitive checksum lines are not the same thing as retained boundary ownership
- Do not preserve duplicate or dead dependencies out of caution.
  - this is greenfield work with no backwards-compatibility requirement
- Do not swallow new lint/test failures behind skips or ignores.
  - every failure must be resolved or the task is not done

## Boundary Review Checklist

- [x] `VerifySourceContract` remains the single support owner for verify-module manifest assertions
- [x] `go.mod` declares Go 1.26
- [x] retained verify-source files do not import `github.com/pkg/errors`
- [x] fetch-only dependency families are blocked at the retained source-import / direct-requirement boundary, not by raw `go.sum` text
- [x] direct dependency bumps are limited to the retained verify slice
- [x] `go mod tidy` does not leave obvious dead direct requirements behind
- [x] no compatibility wrapper or alternate module manifest is introduced

## Final Verification For The Execution Turn

- [x] local Go validation for the retained verify slice on Go 1.26
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] one final `improve-code-boundaries` pass after all lanes are green
- [x] update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

## Execution Discovery Requiring Re-Plan

- `github.com/pkg/errors` cannot be treated as removable from the full module graph based solely on deleting the last retained source import.
  - `GOTOOLCHAIN=auto go mod why -m github.com/pkg/errors` points to retained verify-slice dependencies via `pgconv -> github.com/cockroachdb/apd/v3`.
  - The latest `github.com/cockroachdb/errors` release still requires `github.com/pkg/errors`, so a “nowhere in go.mod/go.sum” contract is structurally wrong.
- Raw `go.sum` string absence is too brittle for the verify manifest boundary.
  - After `go mod tidy`, `go mod why -m golang.org/x/oauth2` can report that the main module does not need `golang.org/x/oauth2`, yet checksum lines can still reappear in `go.sum` through transitive module-history edges.
- Replanning therefore tightens the public contract around actual intent:
  - direct requirements in `go.mod`
  - retained source imports in the verify slice
  - and the explicit Go toolchain declaration
  - not any incidental checksum line in `go.sum`

Plan path: `.ralph/tasks/story-18-verify-http-image/02-task-upgrade-the-verify-slice-to-go-1-26-and-bump-all-dependencies_plans/2026-04-19-verify-go-1-26-dependency-upgrade-plan.md`

NOW EXECUTE
