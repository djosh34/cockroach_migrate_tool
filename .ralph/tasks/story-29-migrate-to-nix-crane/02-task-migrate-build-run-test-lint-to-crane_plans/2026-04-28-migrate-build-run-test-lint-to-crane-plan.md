# Plan: Migrate Build Run Test And Lint To Crane

## References

- Task:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`
- Current workflow surfaces to replace:
  - `Makefile`
  - `Cargo.toml`
  - `crates/runner/Cargo.toml`
  - `cockroachdb_molt/molt/go.mod`
  - `README.md`

## Planning Assumptions

- This turn started with no linked task-02 plan artifact, so this is a planning turn and must stop after the plan is written.
- The repo currently has no `flake.nix`, `flake.lock`, or other Nix source files.
- The current canonical local workflow is still split across two toolchains behind `make`:
  - `make check` => `cargo clippy --workspace --all-targets -- -D warnings`
  - `make lint` => alias of `check`
  - `make test` => `cargo test --workspace` plus `go test ./cmd/verifyservice -count=1` in `cockroachdb_molt/molt`
  - `make test-long` => ignored Rust tests only
- Public execution surfaces already present in the repo and docs:
  - Rust package `runner`
  - Go CLI root `molt`, with the published verify entrypoint exposed as `molt verify-service`
- This is a code and workflow task, so the `tdd` skill applies, but the verification must stay executable.
  - Do not invent brittle file-content tests for Nix expressions.
  - Use tracer-bullet command checks through Nix public interfaces instead.
- No backwards compatibility is allowed.
  - The final tree should have one canonical Nix-native workflow, not a dual-control-plane split between flake outputs and `make`.
  - A thin `make` shim that only delegates to Nix remains acceptable for task completion because the task explicitly allows the old entrypoints to be "removed or replaced"; such a shim is not a second workflow if docs and behavior make Nix the source of truth.

## Approval And Verification Priorities

- Highest-priority behaviors to prove:
  - a fresh checkout can build the Rust workspace through crane-backed Nix outputs
  - the Rust test and lint lanes are runnable through Nix and fail loudly on real failures
  - the Go verify-service test lane remains reachable through Nix
  - the documented runtime entrypoints for `runner` and `verify-service` are reachable through Nix apps/packages
  - dependency artifacts are split from source artifacts so a code-only change does not force a full dependency rebuild
  - `make lint`, `make test`, and the old Make-centric workflow are removed as the canonical path
- Lower-priority behaviors:
  - adding extra developer ergonomics beyond what the task explicitly requests
  - introducing optional parallel wrappers before the basic flake surfaces are stable

## Current State Summary

- Rust workspace members:
  - `ingest-contract`
  - `operator-log`
  - `runner`
- Go module:
  - `cockroachdb_molt/molt`
- Current docs still teach image usage, but local developer validation is not Nix-backed yet.
- There is no existing Nix boundary at all, so task 02 must establish it cleanly rather than layering wrappers over Cargo and Go commands.

## Improve-Code-Boundaries Focus

- Primary boundary smell in the current tree:
  - `Makefile` is acting as the canonical developer interface while delegating to multiple toolchains and directories.
  - That creates an unnecessary orchestration layer with duplicated semantics instead of one explicit build graph.
- Required boundary flattening during execution:
  - move canonical build, app, lint, test, and long-test surfaces into `flake.nix`
  - keep Rust build logic in crane-backed helpers
  - keep Go verification logic in a dedicated Nix helper for the Go module instead of shoving it through Rust/crane abstractions
  - remove the old Make entrypoints once the flake exposes the real public interfaces
- Preferred boundary shape after execution:
  - `flake.nix` becomes the single source of truth for local build orchestration
  - crane owns Rust dependency/source layering
  - one small Nix helper owns the Go verify-service lane
  - docs and task notes reference Nix commands only

## Proposed Public Nix Interface

- `packages.runner`
  - crane-built Rust binary package for the `runner` crate
- `packages.verify-service`
  - Go-built package exposing the `molt verify-service` runtime surface
- `apps.runner`
  - `nix run .#runner -- <runner args>`
- `apps.verify-service`
  - `nix run .#verify-service -- <verify-service args>`
- `checks`
  - `runner-clippy`
  - `runner-test`
  - `verify-service-test`
  - `fmt-check`
  - `long-test`
- convenience commands
  - `nix build .#runner`
  - `nix flake check`
  - `nix run .#lint`
  - `nix run .#test`
  - `nix run .#test-long`
  - `nix run .#fmt`
- `devShells.default`
  - toolchain shell containing the exact Rust, Go, and Nix-facing tools required by the flake commands

## TDD Execution Strategy

- Tracer bullet first:
  - add the minimal flake that can successfully build one real Rust package through crane, ideally `packages.runner`
  - prove it with `nix build .#runner`
- Then add one executable behavior at a time:
  - RED: expose one missing Nix public command or check and run it so it fails
  - GREEN: add the minimum flake code/helper needed for that one command to pass
  - repeat for Rust lint, Rust tests, Go verify-service tests, apps, format, and long-test
- Refactor only after those behaviors are passing:
  - extract common crane helper wiring
  - extract Go helper wiring
  - remove Make-based indirection
- Do not write fake tests against file contents or string presence in the flake.
  - Every verification step must execute the real command surface.

## Intended Files And Surfaces To Change During Execution

- New:
  - `flake.nix`
  - `flake.lock`
- Existing repo files likely to change:
  - `README.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`
- Existing repo files likely to be removed or heavily reduced:
  - `Makefile`
- Possible support files only if execution proves they reduce duplication rather than add indirection:
  - a small `nix/` helper directory
- Explicitly avoid:
  - parallel legacy wrappers that preserve `make` as a second canonical interface
  - shell scripts that swallow exit codes
  - fake compatibility aliases that hide whether Nix is really in control

## Execution Slices

### Slice 1: Establish The Flake Skeleton

- [ ] Add `flake.nix` and `flake.lock`.
- [ ] Pin `nixpkgs`, `flake-utils`, and `crane`.
- [ ] Create one shared `pkgs` / `craneLib` setup for the supported system.
- [ ] Define a cleaned Rust source and a separate Cargo dependency artifact derivation.
- [ ] Tracer-bullet verification:
  - run `nix build .#runner`
  - if this fails, do not add more surfaces until the basic crane package works

### Slice 2: Separate Dependency And Source Builds Properly

- [ ] Use crane's dependency/source split so Rust dependencies are built once and reused by downstream Rust derivations.
- [ ] Make the Rust package, clippy lane, test lane, and long-test lane all consume the same dependency artifact base where crane supports it.
- [ ] Record the concrete derivation/output path used to represent dependency artifacts in task notes.
- [ ] Verification:
  - build the dependency artifact and `runner`
  - apply a temporary code-only source edit in a Rust source file
  - rebuild and confirm the dependency artifact output path stays unchanged while the source derivation rebuilds
  - remove the temporary edit before final checks

### Slice 3: Expose Rust Build, Lint, Test, And Long-Test Through Nix

- [ ] Add Nix checks or runnable commands for:
  - clippy with `-D warnings`
  - workspace tests
  - ignored Rust tests for the long lane
- [ ] Ensure the long lane is exposed but not part of the normal task-end validation flow unless the story/task explicitly requires it.
- [ ] Ensure the failure surface is honest:
  - a clippy failure must fail the Nix command directly
  - a Rust test failure must fail the Nix command directly
- [ ] Verification:
  - `nix run .#lint`
  - `nix run .#test`
  - `nix run .#test-long` only if needed for this task's explicit acceptance, otherwise just ensure the surface exists and leave story-end execution to later

### Slice 4: Bring The Go Verify-Service Lane Into The Flake

- [ ] Add a dedicated Go package/helper rooted at `cockroachdb_molt/molt`.
- [ ] Expose the verify runtime as a Nix package/app that maps cleanly to `molt verify-service`.
- [ ] Add a Nix test lane for `go test ./cmd/verifyservice -count=1`.
- [ ] Keep Go packaging separate from the crane helper so toolchain boundaries stay obvious.
- [ ] Verification:
  - `nix build .#verify-service`
  - `nix run .#test` must include the verify-service Go lane or a clearly documented equivalent aggregate command

### Slice 5: Expose Runtime Apps And Format Surface

- [ ] Expose `apps.runner` for the documented `validate-config` and `run` CLI flows.
- [ ] Expose `apps.verify-service` so the verify runtime is reachable without Docker.
- [ ] Add a formatting surface.
  - likely `cargo fmt --all --check` for validation plus an optional write-mode formatter command if useful
  - if Go formatting is in scope, use a real Go formatter command rather than a text assertion
- [ ] Verification:
  - run one real `nix run` invocation for each app with `--help` or equivalent non-destructive public CLI surface
  - run the format check command through Nix

### Slice 6: Remove Make As The Canonical Orchestration Layer

- [ ] Remove `Makefile` entirely if the flake covers all required public commands cleanly.
- [ ] If repo-level completion gates still require `make check`, `make lint`, and `make test`, keep only a tiny non-canonical shim that delegates straight to the Nix public commands with no duplicate logic.
- [ ] Update any docs/task notes that still point developers to `make`.
- [ ] Verification:
  - search the repo for `make check`, `make lint`, `make test`, and stale local-workflow references
  - rewrite docs to the new Nix commands only

### Slice 7: Final Validation And Boundary Review

- [ ] Run `make check` and `make lint` only after replacing them with the canonical Nix-backed behavior or removing the Make dependency entirely.
- [ ] Run `make test` only after it is intentionally wired to the Nix-backed aggregate test path, or after confirming the repo-level policy has been updated to accept the direct Nix replacement.
- [ ] Run the mandatory final gates for this task:
  - `make check`
  - `make lint`
  - `make test`
  - while respecting the requirement that the old Make-centric workflow is removed or fully replaced
- [ ] Update task notes with:
  - canonical Nix commands
  - dependency reuse evidence
  - exact validation commands run
  - whether `Makefile` was removed or replaced with a Nix-backed shim
- [ ] Do one final `improve-code-boundaries` pass:
  - confirm the repo no longer has two canonical local workflow layers
  - confirm Go and Rust toolchain logic live behind small explicit Nix helpers rather than cross-calling wrappers

## Expected Outcome

- The repo gains a real `flake.nix` using crane for Rust builds with dependency reuse.
- The Go verify-service lane becomes part of the same Nix-native developer workflow without pretending it is Rust/crane work.
- The canonical local interface becomes Nix-based build/run/lint/test commands with docs to match.
- The Make-centric orchestration layer is either removed or reduced to a trivial Nix-backed shim with no independent behavior.

Plan path: `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane_plans/2026-04-28-migrate-build-run-test-lint-to-crane-plan.md`

NOW EXECUTE
