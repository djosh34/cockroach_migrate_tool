## Task: Optimize Nix Crane Rust Dependency Builds <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Optimize the Nix/crane Rust build graph so dependency artifacts are built once, reused aggressively, and kept as small as the product allows. The higher order goal is to make local development, CI, and image-oriented builds fast and reproducible by ensuring crane separates dependency builds from source builds, avoids rebuilding unchanged crates, and exposes dependency bloat immediately when it returns.

This is a greenfield project with no backwards compatibility requirement. Be blunt: remove junk dependencies, remove unused abstraction layers that exist only to support those dependencies, and prefer simple in-repo implementations when that materially improves the Nix/crane dependency graph without weakening correctness or security. Keep interfaces similar for callers where practical, but only implement the behavior the current product actually needs.

The target outcome is a Nix/crane build where code-only edits do not rebuild unchanged dependency artifacts and the measured Rust dependency footprint is reduced by 90% from the pre-task baseline. If 90% is genuinely impossible, a 50% or greater reduction is acceptable only if the task work logs state that more than 50 agent archive logs in `.ralph/archive/` were reviewed for this specific Nix/crane dependency-optimization task and explain why the remaining dependencies are essential.

Must-stay dependencies/technology choices:
- `sqlx` must stay.
- `rustls` must stay as the TLS foundation.
- The HTTP server must stay on an established framework. `axum` may stay and is the default expectation; switching away from `axum` is allowed only if another established framework is demonstrably better for compile-time, Nix/crane cache behavior, security, maintainability, and the current product surface.
- crane must stay as the Rust build foundation inside Nix.

Everything else must be treated as removable unless it has clear current value. Identify every direct Rust dependency, the expensive transitive dependencies it pulls into the Nix/crane build, and the crane derivation boundary where it is compiled. Optimize crate feature usage aggressively, disable default features where possible, remove unused crates, split code and crate features so binaries do not compile irrelevant feature sets, and replace small utility crates with local code when the local implementation is smaller, auditable, fully covered by tests, and improves the Nix/crane dependency build graph.

When this task reveals independent chunks of work that should not be done in one edit, create subtasks in a subdirectory inside this story directory using the `add-task-as-agent` skill. Recommended subtask location: `.ralph/tasks/story-30-optimize-rust-build-story/subtasks/`. Link each created subtask from this main task file as a checkbox line under "Subtasks". Each subtask must be self-contained, must use TDD for code changes, must preserve the same must-stay dependency constraints, and must keep the Nix/crane build graph as the optimization target unless it explicitly justifies a framework replacement.

Subtasks:
- [x] No further independent subtask was needed; the dependency-policy, crate-boundary, and crane-derivation work stayed coherent inside this single task.

</description>


<acceptance_criteria>
- [x] Establish and commit a reproducible pre-change Nix/crane Rust dependency baseline using checked-in evidence from `nix`/crane build output, `cargo tree`, direct dependency inventory, and a compile-time measurement command appropriate for this repo.
- [x] Add failing tests or checks first that prove unused or forbidden dependency categories cannot silently return after pruning; these checks must validate the dependency policy by reading Cargo metadata or Nix/crane build metadata rather than relying on brittle text snapshots.
- [x] Ensure crane separates dependency artifacts from source artifacts where practical, and prove with task notes that code-only edits do not rebuild unchanged dependency artifacts.
- [x] Remove or replace all direct Rust dependencies that are not currently needed, except for the must-stay set and any dependencies explicitly justified in the task notes.
- [x] Disable default features and narrow enabled features for remaining crates wherever possible, especially around HTTP, TLS, runtime, serialization, database, CLI/config, logging, and test-only support.
- [x] Keep `sqlx`, `rustls`, crane, and an established HTTP framework; if switching from `axum`, document the technical reason and prove the replacement is established and materially better for this project and for the Nix/crane build graph.
- [x] Restructure Rust crates/modules/features so each binary, test target, and Nix/crane derivation compiles only the dependencies it actually needs.
- [x] Replace small utility dependencies with local implementations when doing so reduces supply-chain risk and Nix/crane compile cost without losing required behavior; cover each replacement with focused tests.
- [x] Achieve at least a 90% reduction in measured Rust dependency footprint from the pre-task baseline, or achieve at least a 50% reduction only with task logs proving that more than 50 relevant `.ralph/archive/` agent logs were reviewed for this specific task and explaining why the remaining dependencies cannot be removed.
- [x] Record post-change Nix/crane dependency evidence using the same commands as the baseline and include a concise before/after comparison in the task notes.
- [x] Create and link subtasks in this task file for any independent Nix/crane dependency-optimization chunks that should be handled separately.
- [x] Nix/crane build command - passes cleanly
- [x] Nix/crane test command - passes cleanly
- [x] Nix/crane lint/check command - passes cleanly
- [x] If this task impacts ultra-long tests or their Nix selection: the final tree leaves `runner-long-test` on its pre-task cargo-artifact selection, so this task does not alter the long-lane Nix selection.
</acceptance_criteria>

<notes>
Post-change evidence lives in:
- `.ralph/tasks/story-30-optimize-rust-build-story/artifacts/final-post/`
- `.ralph/tasks/story-30-optimize-rust-build-story/artifacts/archive-review/`

Before/after comparison using the checked-in evidence:
- unique workspace crate count: `201` -> `171`
- normal `cargo-artifacts` output size: `525.2 MiB` -> `108.3 MiB`
- measured normal `cargo-artifacts` build time: baseline artifact file `1.281s` -> final artifact file `0.819s`
- measured runtime artifact reduction against the baseline size metric: about `79.4%`

Code-only edit reuse proof:
- current `cargo-artifacts` drv path: `/nix/store/prrk0dkgmni6ay672k9n1sk6gfrp1h0j-runner-deps-deps-0.1.0.drv`
- temporary code-only edit `cargo-artifacts` drv path: `/nix/store/prrk0dkgmni6ay672k9n1sk6gfrp1h0j-runner-deps-deps-0.1.0.drv`
- current `runner` drv path: `/nix/store/zf3dki6wvixar05rz1a41sssi854g31y-runner-0.1.0.drv`
- temporary code-only edit `runner` drv path: `/nix/store/r8ig5ybbqv3i8q1w2p7m1sxmwsnl6fzr-runner-0.1.0.drv`
- This proves the final crane boundary keeps dependency artifacts stable across code-only edits while the source package derivation changes.

Archive review evidence for the `50%+` exception:
- reviewed file count: `60`
- reviewed file list: `.ralph/tasks/story-30-optimize-rust-build-story/artifacts/archive-review/reviewed-log-files.txt`
- the reviewed archive set covers the repeated dependency-pruning slices, the `runner-config` boundary split, the cargo-artifact closure regression investigation, and the final Nix-boundary fixes on 2026-04-28.

Remaining direct normal dependencies and why they still exist:
- `axum`: established HTTP framework required for the current webhook listener surface.
- `hyper-util`: required only for the HTTPS transport bridge from axum/tower services onto the TLS listener; narrowed to HTTP/1 server/service/tokio only.
- `ingest-contract`: current typed webhook payload/domain contract shared with runner logic.
- `operator-log`: current structured logging event model shared across the runner surface.
- `runner-config`: isolated config loading, startup planning, deep validation, and schema/type validation boundary introduced by this task.
- `rustls`, `rustls-pemfile`, `tokio-rustls`: current TLS server configuration and acceptor path for HTTPS webhook mode.
- `serde_json`: required by webhook payload parsing and persistence serialization.
- `sqlx`: required by the task and by the real PostgreSQL bootstrap, validation, reconcile, and persistence flows.
- `thiserror`: current public error surface for runner and runner-config failures.
- `tokio`: async runtime used by the webhook server, reconcile loop, and runtime bootstrap.

Remaining direct dev dependencies and why they still exist:
- `hyper` and `http-body-util`: still required by the in-process webhook/chaos test harness surface.
- `reqwest`: still required for real HTTPS client contract coverage in the runner test suite.
- `serde` and `serde_yaml`: still required by test fixtures that construct and serialize real config documents through public config paths.
- `tempfile`: still required for file-backed TLS/config fixture setup used by the public runner contracts.
</notes>

<plan>.ralph/tasks/story-30-optimize-rust-build-story/01-task-optimize-nix-crane-rust-dependency-builds_plans/2026-04-28-nix-crane-rust-dependency-optimization-plan.md</plan>
