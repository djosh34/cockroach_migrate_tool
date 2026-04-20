# Plan: Verify Structured JSON Logging Across All Shipped Images

## References

- Task:
  - `.ralph/tasks/story-22-structured-json-logging/01-task-verify-all-images-support-structured-json-logging.md`
- Supported operator-facing image docs and compose contracts:
  - `README.md`
  - `artifacts/compose/runner.compose.yml`
  - `artifacts/compose/setup-sql.compose.yml`
  - `artifacts/compose/verify.compose.yml`
- Current shipped image build/runtime contracts:
  - `Dockerfile`
  - `crates/setup-sql/Dockerfile`
  - `crates/runner/tests/image_contract.rs`
  - `crates/setup-sql/tests/image_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
- Current command entrypoints and logging seams:
  - `crates/runner/src/main.rs`
  - `crates/runner/src/lib.rs`
  - `crates/setup-sql/src/main.rs`
  - `crates/setup-sql/src/lib.rs`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- Existing contract and harness tests to extend:
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/support/runner_process.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/setup-sql/tests/cli_contract.rs`
  - `crates/setup-sql/tests/bootstrap_contract.rs`
  - `crates/setup-sql/tests/support/source_bootstrap_image_harness.rs`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public-interface direction in this turn.
- This turn is planning-only because the task file had no execution marker yet.
- The supported shipped product surface is exactly three published images:
  - `runner`
  - `setup-sql`
  - `verify`
- The repository-root `Dockerfile` is not a fourth operator-facing image.
  - It is the build contract for the shipped `runner` image, not a distinct supported image name.
- Structured JSON logging must cover both normal informational events and failure events.
- Artifact payloads must stay machine-usable.
  - `setup-sql emit-*` stdout remains the SQL or JSON artifact payload.
  - Structured logs for that image must therefore live on stderr, not be mixed into stdout.
- If the first RED slice proves that one shared activation path cannot be applied honestly across all three images without colliding with the existing payload/output contract, this plan must stay `TO BE VERIFIED` and execution must stop immediately.

## Supported Image Inventory

- `runner`
  - Documented in `README.md` Docker quick start and `artifacts/compose/runner.compose.yml`
  - Built from `Dockerfile`
  - Public command surface is `runner validate-config` and `runner run`
- `setup-sql`
  - Documented in `README.md` setup quick start and `artifacts/compose/setup-sql.compose.yml`
  - Built from `crates/setup-sql/Dockerfile`
  - Public command surface is `setup-sql emit-cockroach-sql` and `setup-sql emit-postgres-grants`
- `verify`
  - Documented in `README.md` verify compose quick start and `artifacts/compose/verify.compose.yml`
  - Built and exercised by `crates/runner/tests/verify_image_contract.rs`
  - Public command surface is `molt verify-service validate-config` and `molt verify-service run`

## Current State Summary

- `runner` does not have a structured logging path today.
  - `crates/runner/src/main.rs` prints success text with `println!` and failures with `eprintln!`.
  - `runner validate-config` returns a human-only summary string from `Display`.
- `setup-sql` does not have a structured logging path today.
  - `crates/setup-sql/src/main.rs` prints command output with `println!` and failures with `eprintln!`.
  - This currently conflates command payload delivery with operator-facing logging.
- `verify` is only partially compliant today.
  - `verify-service run` builds a `zerolog` logger, so runtime log lines can already be JSON-shaped.
  - `verify-service validate-config` still prints plain text via `fmt.Fprintf`.
  - The activation path is inconsistent because only the run path has logger wiring.
- The main boundary smell from `improve-code-boundaries` is not "missing JSON serializers".
  - The real smell is that payload rendering, command-result reporting, and runtime logging are mixed together differently in each image.
  - If execution patches each image independently, the repo will gain three different activation paths and three subtly different JSON contracts.

## Boundary Decision

- Introduce one operator-facing structured logging activation contract shared by all shipped images:
  - `--log-format text|json`
- Do not reuse `--format`.
  - `setup-sql --format` already controls artifact payload shape on stdout.
  - Reusing it for logs would muddy the payload/logging boundary and force per-image exceptions.
- Reserve stdout for command payloads only.
  - `setup-sql emit-*` keeps stdout for the rendered artifact.
  - `runner validate-config` and `verify-service validate-config` may emit no stdout payload in JSON mode if all operator-facing output is represented on stderr as structured logs.
- Standardize operator log lines as one JSON object per line on stderr for all images in `--log-format json`.
- Keep text mode available for local readability, but treat JSON mode as the supported operator collection path.
- Mirror one shared event shape across Rust and Go instead of allowing framework defaults to diverge.
  - Each emitted line should at minimum carry stable keys for:
    - `timestamp`
    - `level`
    - `service`
    - `event`
    - `message`
  - Command-specific fields can be added as extra keys, but the base keys must not drift per image.

## Public Contract To Establish

- Every shipped image accepts `--log-format json` on its public commands without changing image-specific parser rules.
- In JSON mode, operator-facing logs are emitted as line-delimited JSON objects on stderr only.
- No command emits mixed human text plus JSON fragments on the same logging stream in JSON mode.
- `setup-sql emit-*` preserves stdout as payload-only output even when JSON logging is enabled.
- `runner validate-config` and `verify-service validate-config` expose normal success events and validation failure events through JSON logs.
- `runner run` and `verify-service run` expose at least one startup/info event and one explicit failure event through JSON logs.
- The base keys needed by log shippers remain consistent across `runner`, `setup-sql`, and `verify`.

## Proposed Module Shape

### Rust Side

- Add one small shared Rust logging contract crate or shared module boundary for the two Rust images.
  - Likely direction:
    - a tiny workspace crate such as `crates/operator-log`
  - It should own:
    - `LogFormat`
    - command-line parsing helper reuse where practical
    - the stable JSON event envelope
    - stderr writing helpers for info and error events
- `runner` and `setup-sql` should stop formatting ad hoc success/error text in `main`.
  - `main` becomes a thin boundary that:
    - parses CLI including `--log-format`
    - calls the command implementation
    - writes success/error events through the logging contract
    - preserves stdout payloads only where the command truly returns a payload
- Avoid building a second DTO graph for logs if a single `LogEvent` envelope plus serde value fields is enough.

### Go Side

- Keep `zerolog` for `verify-service run`, but centralize logger construction behind one explicit `LogFormat` choice.
- Remove direct `fmt.Fprintf` success summaries from `validate-config`.
  - `validate-config` should report success/failure through the same structured logger contract when JSON mode is enabled.
- If necessary, add a tiny helper in `cockroachdb_molt/molt/cmd/verifyservice` that:
  - parses `--log-format`
  - configures text or JSON logger output consistently
  - emits command-result events with the same base fields as the Rust images

## Expected Boundary Flattening

- Remove the current split where:
  - Rust binaries treat command output as logs
  - Go runtime uses a logger but Go validate-config prints text directly
- Replace it with one clean rule:
  - payload goes to stdout only when the command has a payload
  - operator logs always go to stderr
  - JSON mode always means JSON lines on stderr
- This should let us delete or simplify:
  - `runner::ValidatedConfig` `Display`-only reporting as the sole operator contract
  - direct `println!` / `eprintln!` ownership in Rust `main`
  - direct `fmt.Fprintf` success output in Go validate-config

## TDD Slices

### Slice 1: Tracer Bullet For Runner JSON Logging

- RED:
  - add a runner CLI contract test for `runner validate-config --log-format json --config ...`
  - assert:
    - stderr contains exactly one line-delimited JSON object for a successful validation event
    - the JSON object has stable base keys:
      - `timestamp`
      - `level`
      - `service`
      - `event`
      - `message`
    - `service == "runner"`
    - stderr does not contain plain-text prefix fragments like `config valid:`
    - stdout is empty in JSON logging mode
- GREEN:
  - add `--log-format` to runner command parsing
  - route validation success and failure reporting through the shared logging boundary
- REFACTOR:
  - keep `main` thin and remove any remaining ad hoc string formatting from it

### Slice 2: Setup-SQL Preserves Payloads While Logging In JSON

- RED:
  - add a command test for `setup-sql emit-cockroach-sql --log-format json --config ...`
  - assert:
    - stdout remains the SQL artifact only
    - stderr contains one valid JSON log line for the successful command event
    - stderr has no mixed human prose outside the JSON object
  - add a failure-path test with invalid config that asserts:
    - stdout stays empty
    - stderr is one JSON error event with explicit failure details
- GREEN:
  - extend setup-sql CLI parsing with `--log-format`
  - split payload emission from operator log emission cleanly
- REFACTOR:
  - keep output rendering in the existing render layer and move operator log formatting fully out of command execution

### Slice 3: Verify-Service Validate-Config Joins The Same Contract

- RED:
  - extend `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - assert `verify-service validate-config --log-format json --config ...` emits:
    - one valid JSON success event on stderr or the command logger stream chosen for logs
    - no old multi-line human summary text
  - add an invalid-config test that asserts one explicit JSON error event
- GREEN:
  - add `--log-format` to verify-service commands
  - replace `fmt.Fprintf` success reporting with the structured logger path
- REFACTOR:
  - keep command-result event emission in one helper so run/validate-config cannot drift

### Slice 4: Runtime Info And Failure Events Stay Structured

- RED:
  - add image-backed or process-backed runtime tests for:
    - `runner run --log-format json --config ...`
    - `verify-service run --log-format json --config ...`
  - assert:
    - normal startup emits at least one valid JSON info event
    - a controlled startup failure emits at least one valid JSON error event
    - no non-JSON log lines appear on the log stream in JSON mode
- GREEN:
  - wire runtime startup/failure event logging through the same contract
  - ensure internal logger initialization does not fall back to human-only lines before the runtime starts
- REFACTOR:
  - if runtime tests reveal duplicate startup/failure logging in different modules, collapse it into one startup boundary

### Slice 5: Image-Level Contract For All Shipped Images

- RED:
  - extend image harnesses or add new image contract tests that prove the shipped images themselves accept the JSON logging path:
    - runner image
    - setup-sql image
    - verify image
  - assert at least one success case per image where logs parse as line-delimited JSON objects
- GREEN:
  - update image invocation/tests so the new flag survives entrypoint boundaries and does not get swallowed by shell glue
- REFACTOR:
  - keep image-level tests thin; they should prove the shipped surface, not re-test every internal field

### Slice 6: Surface Stability And Documentation

- RED:
  - add or extend contract tests that protect the chosen activation path from drifting:
    - `--log-format`
    - stderr-only JSON log stream
    - no per-image parser exceptions
  - update README and compose examples only after tests force the desired public contract
- GREEN:
  - document the supported activation path consistently in:
    - `README.md`
    - `artifacts/compose/*.compose.yml` comments or examples if needed
- REFACTOR:
  - keep docs aligned with the tested surface and remove any stale plain-text-only guidance

## Bug Handling Rule During Execution

- If investigation during execution shows that a shipped image still emits unavoidable non-JSON lines in JSON mode from a deeper dependency that cannot be corrected inside this task, stop and create a bug immediately with the `add-bug` skill.
- If that happens, request a task switch exactly as the task requires.

## Expected File Touches During Execution

- Task metadata:
  - `.ralph/tasks/story-22-structured-json-logging/01-task-verify-all-images-support-structured-json-logging.md`
- Rust runner:
  - `crates/runner/src/main.rs`
  - `crates/runner/src/lib.rs`
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/support/runner_process.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
- Rust setup-sql:
  - `crates/setup-sql/src/main.rs`
  - `crates/setup-sql/src/lib.rs`
  - `crates/setup-sql/tests/cli_contract.rs`
  - `crates/setup-sql/tests/bootstrap_contract.rs`
  - `crates/setup-sql/tests/support/source_bootstrap_image_harness.rs`
- Shared Rust logging boundary:
  - likely a new crate or shared module such as `crates/operator-log/...`
- Go verify-service:
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - possibly `cockroachdb_molt/molt/verifyservice/runtime.go`
  - possibly `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- Docs/contracts:
  - `README.md`
  - image/compose contract tests if they need to pin the new activation path

## Execution Checklist

- [x] RED/GREEN slice 1 for runner validate-config JSON logging
- [x] RED/GREEN slice 2 for setup-sql payload-plus-logging separation
- [x] RED/GREEN slice 3 for verify-service validate-config JSON logging
- [x] RED/GREEN slice 4 for runtime startup and failure event logging
- [x] RED/GREEN slice 5 for shipped-image contract coverage
- [x] RED/GREEN slice 6 for docs and surface stability
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] Final `improve-code-boundaries` pass to ensure the activation path and event envelope are not re-fragmented per image
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after all required lanes are green

Plan path: `.ralph/tasks/story-22-structured-json-logging/01-task-verify-all-images-support-structured-json-logging_plans/2026-04-20-all-images-structured-json-logging-plan.md`

NOW EXECUTE
