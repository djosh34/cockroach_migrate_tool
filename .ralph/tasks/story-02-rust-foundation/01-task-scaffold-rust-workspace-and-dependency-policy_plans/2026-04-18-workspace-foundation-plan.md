# Plan: Rust Workspace Foundation

## References

- Task: `.ralph/tasks/story-02-rust-foundation/01-task-scaffold-rust-workspace-and-dependency-policy.md`
- Design: `designs/crdb-to-postgres-cdc/05_design_decisions.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Interface And Boundary Decisions

- Keep the workspace small. Do not create a crate per concern.
- Create exactly two first-class application crates:
  - `crates/runner`
  - `crates/source-bootstrap`
- Keep `config`, `error`, `postgres`, `webhook`, and `reconcile` as internal modules of `runner` so the destination runtime stays a deep module instead of a shallow pile of pass-through crates.
- Keep TLS and HTTP concerns inside `runner::webhook`; do not invent a shared transport layer yet.
- Keep configuration reduced at load time. Runtime code should depend on validated config structs, not raw YAML maps or duplicated fields.
- Keep error boundaries typed with `thiserror` and `#[from]`; do not use string buckets.
- Use workspace-level dependency policy and lints so every crate inherits the same contract.

## Public Contract To Establish

- `cargo check --workspace`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` are the canonical Rust commands behind `make check`, `make test`, and `make lint`.
- `make test-long` exists now and runs the long-test lane cleanly even if this story does not introduce long tests yet.
- `runner` exposes a stable CLI contract:
  - `runner validate-config --config <path>`
  - `runner run --config <path>`
- `source-bootstrap` exposes a stable CLI contract:
  - `source-bootstrap create-changefeed --config <path>`
- Both binaries load real YAML config through typed structs and fail loudly on invalid input.

## Files And Structure To Add

- [x] Root `Cargo.toml` workspace with:
  - members for `crates/runner` and `crates/source-bootstrap`
  - workspace package metadata
  - workspace dependencies for `anyhow` only if needed at the binary edge, `axum`, `clap`, `serde`, `serde_yaml`, `sqlx`, `thiserror`, `tokio`, and TLS crates actually needed for this story
  - workspace lint settings that deny warnings
- [x] Root `Makefile` with `check`, `lint`, `test`, and `test-long`
- [x] Root `README.md` section documenting:
  - workspace purpose
  - command contract
  - dependency policy that `sqlx`, `thiserror`, established HTTP/TLS/config/CLI crates are mandatory choices and hand-rolled replacements are not allowed
- [x] `crates/runner/Cargo.toml`
- [x] `crates/runner/src/lib.rs`
- [x] `crates/runner/src/main.rs`
- [x] `crates/runner/src/config.rs`
- [x] `crates/runner/src/error.rs`
- [x] `crates/runner/src/postgres.rs`
- [x] `crates/runner/src/webhook.rs`
- [x] `crates/runner/src/reconcile.rs`
- [x] `crates/source-bootstrap/Cargo.toml`
- [x] `crates/source-bootstrap/src/lib.rs`
- [x] `crates/source-bootstrap/src/main.rs`
- [x] `crates/source-bootstrap/src/config.rs`
- [x] `crates/source-bootstrap/src/error.rs`
- [x] Example config fixtures under `crates/runner/tests/fixtures/` and `crates/source-bootstrap/tests/fixtures/`

## TDD Execution Order

### Slice 1: Workspace Command Contract

- [x] RED: add an integration-style test that invokes `runner --help` and fails because the crate/binary does not exist yet
- [x] GREEN: add the minimal workspace, `runner` crate, and clap wiring to make the help contract pass
- [x] REFACTOR: move CLI parsing into a small public entrypoint and keep `main.rs` thin

### Slice 2: Source Bootstrap Command Contract

- [x] RED: add an integration-style test that invokes `source-bootstrap --help` and expects `create-changefeed`
- [x] GREEN: add the `source-bootstrap` crate and minimal clap wiring to satisfy the command contract
- [x] REFACTOR: keep source bootstrap config and command parsing in the source crate only; do not share half-baked config types with `runner`

### Slice 3: Typed Config Loading

- [x] RED: add one test for `runner validate-config --config <fixture>` succeeding on a minimal valid config fixture
- [x] GREEN: implement reduced validated config types for runner and load YAML via `serde` / `serde_yaml`
- [x] REFACTOR: remove duplicated config fields from runtime constructors so runtime depends on one validated config object

### Slice 4: Loud Failure On Invalid Config

- [x] RED: add one test for `runner validate-config --config <bad fixture>` failing with a typed error path
- [x] GREEN: implement `thiserror`-based error enums with `#[from]` conversions instead of formatted strings
- [x] REFACTOR: keep validation logic in `config.rs`, not spread across CLI and runtime modules

### Slice 5: Baseline Runner Wiring

- [x] RED: add one test for `runner run --config <fixture>` reaching a baseline startup summary without starting real webhook or reconcile work
- [x] GREEN: implement a minimal `RunnerApp` builder that wires together `config`, `postgres`, `webhook`, and `reconcile` modules behind one constructor
- [x] REFACTOR: keep transport setup inside `webhook` and database setup inside `postgres`; the top-level app should orchestrate, not courier internals

### Slice 6: Baseline Source Bootstrap Wiring

- [x] RED: add one test for `source-bootstrap create-changefeed --config <fixture>` reaching a baseline summary that proves config and command wiring are real
- [x] GREEN: implement a minimal bootstrap application boundary with typed config and typed errors
- [x] REFACTOR: keep bootstrap-specific config and output inside the source crate; do not create a premature shared crate

### Slice 7: Repository Command Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix the first failing lane
- [x] GREEN: finish the missing make wiring and crate configuration until every lane passes
- [x] REFACTOR: remove any temporary helper code or duplicate command wrappers uncovered while making the lanes clean

## Boundary Review Checklist

- [x] No module exists only to forward calls elsewhere
- [x] No raw config data leaks past config loading
- [x] No stringly typed error conversion remains where `#[from]` can be used
- [x] No shared DTO layer exists between `runner` and `source-bootstrap` without a proven need
- [x] No TLS helper lives outside the webhook boundary
- [x] No new library is introduced unless this task directly needs it

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final improve-code-boundaries pass after all tests are green
- [x] Update the task file checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
