## Task: Drastically Reduce Rust Dependency Footprint And Compile Time <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Drastically reduce Rust compile time by reducing dependency count, pruning unnecessary crates, and restructuring code so the dependency graph reflects only what the product actually uses. The higher order goal is to improve developer feedback speed, reduce CI/image build time, and reduce supply-chain/security risk by keeping the Rust dependency surface intentionally small.

This is a greenfield project with no backwards compatibility requirement. Be blunt: remove junk dependencies, remove unused abstraction layers that exist only to support those dependencies, and prefer simple in-repo implementations when that materially reduces the dependency graph without weakening correctness or security. Keep interfaces similar for callers where practical, but only implement the behavior the current product actually needs.

The target outcome is a 90% reduction in Rust dependency footprint from the pre-task baseline. If 90% is genuinely impossible, a 50% or greater reduction is acceptable only if the task work logs state that more than 50 agent archive logs in `.ralph/archive/` were reviewed for this specific dependency-pruning task and explain why the remaining dependencies are essential.

Must-stay dependencies/technology choices:
- `sqlx` must stay.
- `rustls` must stay as the TLS foundation.
- The HTTP server must stay on an established framework. `axum` may stay and is the default expectation; switching away from `axum` is allowed only if another established framework is demonstrably better for compile-time, security, maintainability, and the current product surface.

Everything else must be treated as removable unless it has clear current value. Identify every direct Rust dependency and the expensive transitive dependencies it pulls into the build. Optimize crate feature usage aggressively, disable default features where possible, remove unused crates, split code so binaries do not compile irrelevant feature sets, and replace small utility crates with local code when the local implementation is smaller, auditable, and fully covered by tests.

When this task reveals independent chunks of work that should not be done in one edit, create subtasks in a subdirectory inside this story directory using the `add-task-as-agent` skill. Recommended subtask location: `.ralph/tasks/story-29-optimize-rust-build-story/subtasks/`. Link each created subtask from this main task file as a checkbox line under "Subtasks". Each subtask must be self-contained, must use TDD for code changes, and must preserve the same must-stay dependency constraints unless it explicitly justifies a framework replacement.

Subtasks:
- [ ] Create focused subtasks in `.ralph/tasks/story-29-optimize-rust-build-story/subtasks/` with `add-task-as-agent` when the dependency-pruning work naturally splits into independently reviewable chunks; link every created subtask here before completing this main task.

</description>


<acceptance_criteria>
- [ ] Establish and commit a reproducible pre-change Rust dependency baseline using checked-in evidence from `cargo tree`, direct dependency inventory, and a compile-time measurement command appropriate for this repo.
- [ ] Add failing tests or checks first that prove unused or forbidden dependency categories cannot silently return after pruning; these checks must validate the dependency policy by reading Cargo metadata rather than relying on brittle text snapshots.
- [ ] Remove or replace all direct Rust dependencies that are not currently needed, except for the must-stay set and any dependencies explicitly justified in the task notes.
- [ ] Disable default features and narrow enabled features for remaining crates wherever possible, especially around HTTP, TLS, runtime, serialization, database, CLI/config, logging, and test-only support.
- [ ] Keep `sqlx`, `rustls`, and an established HTTP framework; if switching from `axum`, document the technical reason and prove the replacement is established and materially better for this project.
- [ ] Restructure Rust crates/modules/features so each binary or test target compiles only the dependencies it actually needs.
- [ ] Replace small utility dependencies with local implementations when doing so reduces supply-chain risk and compile cost without losing required behavior; cover each replacement with focused tests.
- [ ] Achieve at least a 90% reduction in measured Rust dependency footprint from the pre-task baseline, or achieve at least a 50% reduction only with task logs proving that more than 50 relevant `.ralph/archive/` agent logs were reviewed for this specific task and explaining why the remaining dependencies cannot be removed.
- [ ] Record post-change dependency evidence using the same commands as the baseline and include a concise before/after comparison in the task notes.
- [ ] Create and link subtasks in this task file for any independent dependency-pruning chunks that should be handled separately.
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
