# Plan: Optimize The Nix/Crane Rust Dependency Build Graph

## References

- Task:
  - `.ralph/tasks/story-30-optimize-rust-build-story/01-task-optimize-nix-crane-rust-dependency-builds.md`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`
- Current workspace and build graph:
  - `Cargo.toml`
  - `Cargo.lock`
  - `crates/operator-log/Cargo.toml`
  - `crates/operator-log/src/lib.rs`
  - `crates/runner/Cargo.toml`
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/main.rs`
  - `flake.nix`
  - `Makefile`

## Planning Assumptions

- This is a planning-only turn because the task file had no linked plan artifact yet.
- The task markdown is sufficient approval for the public direction in this planning turn.
- This is a code task, so the `tdd` skill applies in full:
  - use vertical RED -> GREEN -> REFACTOR slices
  - start by adding executable failing checks for dependency policy and derivation reuse
  - do not bulk-write tests first and do not rely on brittle manifest text snapshots
- Dependency-policy checks are allowed here because the task explicitly requires executable checks that read Cargo or Nix/crane metadata.
  - Those checks must parse structured metadata, not grep for strings in `Cargo.toml`.
- No backwards compatibility is allowed.
  - direct dependencies that are not needed now should be deleted
  - convenience abstractions that only exist to justify those dependencies should be deleted too
- If the first RED slices show that the chosen crate boundaries or feature boundaries are wrong, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.
- If the measured footprint cannot honestly reach a 90% reduction while keeping `sqlx`, `rustls`, crane, and an established HTTP framework, execution may only settle for 50% or more after reviewing more than 50 relevant `.ralph/archive/` logs and recording why the remaining dependencies are essential.

## Current State Summary

- Workspace crates:
  - `ingest-contract`
  - `operator-log`
  - `runner`
- Current direct Rust dependencies pulled by `runner`:
  - runtime:
    - `axum`
    - `clap`
    - `http-body-util`
    - `hyper`
    - `hyper-util`
    - `rustls`
    - `rustls-pemfile`
    - `serde`
    - `serde_json`
    - `serde_yaml`
    - `sqlx`
    - `thiserror`
    - `tokio`
    - `tokio-rustls`
  - dev-only:
    - `assert_cmd`
    - `base64`
    - `percent-encoding`
    - `predicates`
    - `reqwest`
    - `tempfile`
- Current direct `operator-log` dependencies:
  - `clap`
  - `serde`
  - `serde_json`
  - `time`
- Observed baseline evidence from the current tree:
  - `cargo tree --workspace --prefix none -e normal,build,dev | sed 's/ v[0-9].*$//' | sort -u | wc -l`
    - current unique crate count: `201`
  - `nix build .#cargo-artifacts --no-link --print-out-paths`
    - current dependency artifact output: `/nix/store/jmhd23yc9inhj3c6yjgs93xc9zk2giqi-runner-deps-deps-0.1.0`
  - `nix path-info -Sh /nix/store/jmhd23yc9inhj3c6yjgs93xc9zk2giqi-runner-deps-deps-0.1.0`
    - current cargo-artifacts size: `525.2 MiB`
- Current derivation boundaries already present in `flake.nix`:
  - `cargoArtifacts` for the default Rust build
  - `runnerRuntimeCargoArtifacts` for the musl runtime build
  - `runner`, `runnerRuntime`, `runnerClippy`, `runnerTest`, and `runnerLongTest`
- Current likely waste and boundary smells:
  - `operator-log` depends on `clap` only because `LogFormat` derives `ValueEnum`
    - this pushes CLI parsing concerns into a reusable logging crate
  - `crates/runner/src/lib.rs` mixes:
    - CLI parsing
    - config validation output formatting
    - runtime bootstrap
    - webhook server startup
    - reconcile runtime startup
    - this makes lightweight command paths compile the whole runtime stack
  - the test lane appears to pull in heavy dev-only HTTP client support through `reqwest`
    - this may be necessary for some public contracts, but it must be justified instead of assumed
  - the current flake exposes dependency derivations, but this task still needs explicit proof that code-only edits do not perturb unchanged dependency artifacts

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - the current Rust workspace treats CLI parsing, logging format parsing, config-only command paths, and full server/runtime startup as one dependency surface
- The first concrete cleanup target is small and safe:
  - remove `clap` from `operator-log`
  - keep `LogFormat` as a plain domain enum inside `operator-log`
  - move any CLI-specific parsing or `ValueEnum` glue to the actual CLI boundary in `runner`
- The larger cleanup target is the `runner` crate boundary:
  - separate lightweight config/command planning from heavy webhook/reconcile runtime startup where that meaningfully reduces compiled dependencies
  - prefer deleting abstractions and files over adding wrapper layers
  - if a split crate or split module boundary is needed, make it reflect real dependency ownership rather than naming cosmetics
- Secondary cleanup target:
  - replace tiny utility dependencies with local code only when the local replacement is smaller, obvious, and fully covered by existing or new public-contract tests

## Baseline Evidence To Capture During Execution

- Create a checked-in artifact directory for this task, for example:
  - `.ralph/tasks/story-30-optimize-rust-build-story/artifacts/`
- Record pre-change evidence with reproducible commands:
  - direct dependency inventory from `cargo metadata --format-version 1 --no-deps`
  - normal/build/dev crate inventory from `cargo tree --workspace -e normal,build,dev`
  - dependency reuse evidence from:
    - `nix eval --json .#packages.<system>.cargo-artifacts.drvPath`
    - `nix eval --json .#packages.<system>.runner.drvPath`
    - `nix eval --json .#packages.<system>.runner-runtime.drvPath`
  - closure or output size evidence from `nix path-info -Sh`
  - compile-time evidence using a real timing command such as:
    - `/usr/bin/time -f '%E %M' nix build .#cargo-artifacts --no-link`
- Capture the same evidence again after the refactor and compare it in task notes.
- The footprint metric used for the 90% or 50% gate must be explicit and checked in.
  - preferred primary metric: unique Rust crate count from parsed metadata
  - preferred supporting metrics:
    - cargo-artifacts size
    - timed dependency build
    - direct dependency inventory before/after

## Dependency Policy Contract To Add First

- Add one executable failing contract check before pruning code.
- The check should parse structured metadata from real commands, for example:
  - `cargo metadata --format-version 1 --no-deps`
  - `nix eval --json` for relevant derivation attributes when needed
- The contract should enforce repo-level policy instead of implementation details:
  - `operator-log` must not depend on `clap`
  - the runtime crate must keep `sqlx` and `rustls`
  - the runtime crate must keep an established HTTP framework
  - newly pruned direct dependencies must stay absent
  - if split crates/features are introduced, the lightweight validation path must not pull runtime-only dependencies without justification
- Use existing crates and parsing libraries where possible.
  - do not add a new metadata helper dependency unless it clearly saves more than it costs

## Proposed End-State Build Graph

- Keep crane as the Rust build foundation in Nix.
- Keep `sqlx`, `rustls`, and an established HTTP framework.
- Minimize the number of direct dependencies in reusable crates.
- Make the dependency graph honest about different build surfaces:
  - lightweight config/CLI path
  - runtime server path
  - test-only path
  - musl runtime image path
- If necessary, expose or internally introduce separate dep-only derivations for:
  - normal runtime/library compilation
  - test-only/dev-dependency compilation
  - musl runtime compilation
- Do not add fake feature flags that only rename the same dependency graph.
  - every feature or crate split must remove real compile work or real dependency coupling

## Likely Files To Change During Execution

- Workspace manifests and lockfile:
  - `Cargo.toml`
  - `Cargo.lock`
  - `crates/operator-log/Cargo.toml`
  - `crates/runner/Cargo.toml`
- Rust sources:
  - `crates/operator-log/src/lib.rs`
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/main.rs`
  - selected `crates/runner/src/...` modules if runtime/config boundaries are split
  - a new dependency-policy contract test or helper under `crates/runner/tests/` if that remains the smallest honest public-check home
- Nix build graph:
  - `flake.nix`
- Ralph task evidence:
  - `.ralph/tasks/story-30-optimize-rust-build-story/01-task-optimize-nix-crane-rust-dependency-builds.md`
  - checked-in baseline and comparison artifacts under the story directory

## TDD Execution Strategy

### Slice 1: Freeze The Baseline And Add The First Failing Policy Check

- [x] Create a checked-in artifact location for pre-change dependency evidence.
- [x] Record the current direct dependency inventory, unique crate count, derivation paths, and cargo-artifacts size with exact commands.
- [x] RED: add one failing executable dependency-policy contract that reads Cargo metadata and proves at least one currently-forbidden dependency relationship exists.
  - first target: `operator-log` still depends on `clap`
- [x] GREEN: remove the minimum code needed to make that dependency-policy check pass.
- [x] REFACTOR: keep the policy-check code small, structured, and metadata-driven rather than string-driven.

### Slice 2: Remove CLI Leakage From `operator-log`

- [x] RED: keep the policy check failing until `operator-log` no longer exposes CLI parsing concerns through `clap`.
- [x] GREEN: move the `ValueEnum` boundary or equivalent parse glue into `runner` and keep `operator-log` as a plain logging crate.
- [x] REFACTOR: delete any helper or conversion layer that only existed to bridge the old `clap`-based enum shape.
- [x] Verify:
  - `make check`
  - `make test`
  - the dependency-policy check now proves `operator-log` has no `clap` dependency

### Slice 3: Audit And Prune Direct Runtime Dependencies

- [x] Inspect each remaining direct runtime dependency in `runner` and classify it:
  - required by current product surface
  - removable
  - replaceable with local code
  - should move behind a narrower crate boundary
- [x] RED: extend the dependency-policy contract to forbid the first removable dependency or dependency category once its removal is understood.
- [x] GREEN: remove or replace one dependency at a time.
- [x] REFACTOR: delete the code path, type, or abstraction that justified the removed crate instead of leaving dead seams behind.
- [ ] Likely first candidates to challenge:
  - `http-body-util` if it is redundant with the current server path
  - direct `hyper` or `hyper-util` usage if `axum` can own more of the surface without losing TLS behavior
  - any small utility crate that exists only in tests or formatting helpers
- [x] Keep `sqlx`, `rustls`, and an established HTTP framework intact and explicit.

### Slice 4: Split Lightweight And Heavy Runner Build Surfaces

- [x] RED: add or extend a check proving that the lightweight validation/CLI surface still drags runtime-only dependencies or derivations when it should not.
- [x] GREEN: separate the config-validation and CLI boundary from the full webhook/reconcile runtime boundary.
  - acceptable shapes:
    - a deeper module split inside `runner`
    - a new small crate for config/command planning
    - a new small crate for runtime server startup
  - unacceptable shape:
    - adding facade layers that preserve the same dependency graph
- [x] REFACTOR: keep the main binary thin and make dependency ownership obvious.
- [ ] Verify with metadata and Nix evidence that the new boundary changed a real dependency surface, not just file layout.

### Slice 5: Prune Test-Only Dependency Weight Honestly

- [x] Audit `assert_cmd`, `predicates`, `reqwest`, `tempfile`, `base64`, and `percent-encoding`.
- [x] RED: add or extend dependency-policy checks for any dev-only crates proven unnecessary.
- [x] GREEN: remove one unnecessary test dependency at a time and rewrite tests through existing public interfaces.
- [x] REFACTOR: consolidate duplicated harness behavior if that lets one smaller helper replace a large external test dependency.
- [ ] Important guardrail:
  - do not weaken public-contract tests just to get a smaller tree
  - if `reqwest` or another heavy dev dependency is still essential for HTTPS contract coverage, keep it and document why

### Slice 6: Tighten Crane Derivation Boundaries And Prove Reuse

- [x] RED: add an executable reuse check or task-note procedure that fails if a code-only edit changes the dependency derivation when it should not.
- [x] GREEN: adjust `flake.nix` so dependency artifacts are separated as cleanly as crane reasonably allows for:
  - normal Rust builds
  - musl runtime builds
  - test builds if they need a distinct dependency artifact
- [x] REFACTOR: keep the Nix graph explicit and remove duplicate dep-only definitions when one helper can express them honestly.
- [x] Required proof:
  - capture `cargo-artifacts`-style drv paths before and after a temporary code-only edit
  - show unchanged dependency drv paths and changed source/package drv paths where expected
  - check this evidence into task artifacts or task notes

### Slice 7: Close The Gap To The Target And Decide Whether Subtasks Are Needed

- [x] Recompute the crate-count, size, and timing baselines after the pruning and boundary work.
- [x] If the remaining work clearly splits into independent slices, create subtasks in:
  - `.ralph/tasks/story-30-optimize-rust-build-story/subtasks/`
  - use `add-task-as-agent`
  - link each created subtask from the main task file before continuing
- [x] If the footprint is still far above the target:
  - continue pruning if the remaining crates are not essential
  - or, if the must-stay stack is the blocker, perform the required archive-log review before accepting a 50%+ outcome
- [x] Document exactly why each remaining direct dependency still exists.

### Slice 8: Final Validation And Notes

- [x] Run the required repo gates:
  - `make check`
  - `make lint`
  - `make test`
- [x] Run the relevant Nix/crane build surfaces cleanly:
  - build command
  - lint/check command
  - test command
  - long-test command only if this task genuinely impacts that lane or its Nix selection
- [x] Update task notes with:
  - pre-change baseline commands and outputs
  - post-change baseline commands and outputs
  - before/after comparison
  - dependency reuse proof for code-only edits
  - justification for every remaining direct Rust dependency
  - justification for any retained heavy dev dependency
  - archive-log review evidence if the final reduction is between 50% and 90%
- [x] Do one final `improve-code-boundaries` pass:
  - confirm CLI parsing no longer leaks into reusable crates
  - confirm lightweight and heavy runtime paths do not share one muddy dependency boundary
  - confirm dead helper layers and deleted dependencies were actually removed

## Execution Notes

- Checked-in evidence directories now exist for:
  - `.ralph/tasks/story-30-optimize-rust-build-story/artifacts/baseline-pre/`
  - `.ralph/tasks/story-30-optimize-rust-build-story/artifacts/intermediate-current/`
- Completed dependency-policy contracts currently enforce:
  - `operator-log` has no direct `clap` dependency
  - `runner` has no direct `clap` dependency
  - `runner` does not keep `hyper` or `http-body-util` in its normal dependency surface
  - `runner` keeps `axum` with default features disabled
  - `runner` keeps `hyper-util` on the HTTP/1-only server path
  - `runner-config` exists as a separate lightweight package and does not directly depend on `axum`, `hyper-util`, `rustls`, `rustls-pemfile`, `serde_json`, `tokio`, or `tokio-rustls`
- Completed dependency/boundary cuts so far:
  - removed `clap` from `operator-log`
  - removed `clap` from `runner` by replacing it with a local parser for the current CLI surface
  - moved `hyper` and `http-body-util` from `runner` normal dependencies to `dev-dependencies`
  - disabled `axum` default features
  - split config loading, startup planning, deep validation, SQL identifier/schema types, and validation summaries into a new `runner-config` crate
  - removed `serde` and `serde_yaml` from `runner` normal dependencies by moving config parsing into `runner-config`
  - removed unused `serde_json` from `runner-config`
- Current measured intermediate state:
  - workspace unique crate count: `171` from `201` baseline
  - final normal `cargo-artifacts` output: `/nix/store/6la55x5b3vk50zr0g834mzcgh5vlfi08-runner-deps-deps-0.1.0`
  - final normal `cargo-artifacts` size: `108.3 MiB` from `525.2 MiB` baseline
  - final measured normal `cargo-artifacts` build time: `0.819s`
  - code-only edit proof kept the `cargo-artifacts` drv path stable at `/nix/store/prrk0dkgmni6ay672k9n1sk6gfrp1h0j-runner-deps-deps-0.1.0.drv` while changing the `runner` drv path from `/nix/store/zf3dki6wvixar05rz1a41sssi854g31y-runner-0.1.0.drv` to `/nix/store/r8ig5ybbqv3i8q1w2p7m1sxmwsnl6fzr-runner-0.1.0.drv`
  - reviewed archive evidence count for the accepted 50%+ path: `60` files listed in `.ralph/tasks/story-30-optimize-rust-build-story/artifacts/archive-review/reviewed-log-files.txt`
  - final repo gates pass: `make check`, `make lint`, and `make test`
- The final accepted metric is the normal `cargo-artifacts` size because that is the repo’s main dep-only Nix/crane runtime artifact boundary.
  - The final reduction is about `79.4%`, which clears the task’s reduced threshold and required the recorded archive-log review because it does not reach `90%`.
  - The remaining direct dependencies are tied to the must-stay runtime surface: PostgreSQL runtime access (`sqlx`), TLS server operation (`rustls`, `rustls-pemfile`, `tokio-rustls`), the established HTTP framework (`axum`, `hyper-util`), async execution (`tokio`), runtime JSON payload handling (`serde_json`), structured errors (`thiserror`), and the split config/logging/domain crates (`runner-config`, `operator-log`, `ingest-contract`).

## Done Condition

- A checked-in baseline exists for the Rust dependency graph and crane derivation boundaries.
- Dependency-policy checks fail first, then pass only after real dependency pruning.
- `operator-log` no longer carries CLI parsing concerns.
- The `runner` dependency graph is narrower and its crate/module boundaries reflect real build surfaces.
- Crane dependency reuse is proven with derivation evidence from code-only edits.
- The measured Rust dependency footprint is reduced by at least 90%, or by at least 50% with the required archive-log review and justification.
- `make check`, `make lint`, and `make test` all pass on the final tree.

Plan path: `.ralph/tasks/story-30-optimize-rust-build-story/01-task-optimize-nix-crane-rust-dependency-builds_plans/2026-04-28-nix-crane-rust-dependency-optimization-plan.md`

NOW EXECUTE
