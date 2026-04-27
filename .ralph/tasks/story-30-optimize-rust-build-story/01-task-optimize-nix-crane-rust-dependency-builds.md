## Task: Optimize Nix Crane Rust Dependency Builds <status>not_started</status> <passes>false</passes>

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
- [ ] Create focused subtasks in `.ralph/tasks/story-30-optimize-rust-build-story/subtasks/` with `add-task-as-agent` when the Nix/crane dependency-optimization work naturally splits into independently reviewable chunks; link every created subtask here before completing this main task.

</description>


<acceptance_criteria>
- [ ] Establish and commit a reproducible pre-change Nix/crane Rust dependency baseline using checked-in evidence from `nix`/crane build output, `cargo tree`, direct dependency inventory, and a compile-time measurement command appropriate for this repo.
- [ ] Add failing tests or checks first that prove unused or forbidden dependency categories cannot silently return after pruning; these checks must validate the dependency policy by reading Cargo metadata or Nix/crane build metadata rather than relying on brittle text snapshots.
- [ ] Ensure crane separates dependency artifacts from source artifacts where practical, and prove with task notes that code-only edits do not rebuild unchanged dependency artifacts.
- [ ] Remove or replace all direct Rust dependencies that are not currently needed, except for the must-stay set and any dependencies explicitly justified in the task notes.
- [ ] Disable default features and narrow enabled features for remaining crates wherever possible, especially around HTTP, TLS, runtime, serialization, database, CLI/config, logging, and test-only support.
- [ ] Keep `sqlx`, `rustls`, crane, and an established HTTP framework; if switching from `axum`, document the technical reason and prove the replacement is established and materially better for this project and for the Nix/crane build graph.
- [ ] Restructure Rust crates/modules/features so each binary, test target, and Nix/crane derivation compiles only the dependencies it actually needs.
- [ ] Replace small utility dependencies with local implementations when doing so reduces supply-chain risk and Nix/crane compile cost without losing required behavior; cover each replacement with focused tests.
- [ ] Achieve at least a 90% reduction in measured Rust dependency footprint from the pre-task baseline, or achieve at least a 50% reduction only with task logs proving that more than 50 relevant `.ralph/archive/` agent logs were reviewed for this specific task and explaining why the remaining dependencies cannot be removed.
- [ ] Record post-change Nix/crane dependency evidence using the same commands as the baseline and include a concise before/after comparison in the task notes.
- [ ] Create and link subtasks in this task file for any independent Nix/crane dependency-optimization chunks that should be handled separately.
- [ ] Nix/crane build command - passes cleanly
- [ ] Nix/crane test command - passes cleanly
- [ ] Nix/crane lint/check command - passes cleanly
- [ ] If this task impacts ultra-long tests or their Nix selection: Nix/crane long-test command - passes cleanly
</acceptance_criteria>
