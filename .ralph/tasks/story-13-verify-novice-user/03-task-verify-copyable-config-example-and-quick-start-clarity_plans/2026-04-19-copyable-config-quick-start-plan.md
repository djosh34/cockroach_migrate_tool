# Plan: Verify Copyable Config Example And Quick-Start Clarity

## References

- Task: `.ralph/tasks/story-13-verify-novice-user/03-task-verify-copyable-config-example-and-quick-start-clarity.md`
- Previous story-13 plans:
  - `.ralph/tasks/story-13-verify-novice-user/01-task-verify-readme-alone-is-sufficient-for-novice-user_plans/2026-04-19-readme-novice-user-plan.md`
  - `.ralph/tasks/story-13-verify-novice-user/02-task-verify-direct-docker-build-and-run-without-wrapper-scripts_plans/2026-04-19-direct-docker-build-run-plan.md`
- Current public quick-start docs:
  - `README.md`
- Existing README contract support:
  - `crates/runner/tests/readme_contract.rs`
  - `crates/runner/tests/support/readme_contract.rs`
- Existing runner config contracts:
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/fixtures/valid-runner-config.yml`
  - `crates/runner/tests/fixtures/container-runner-config.yml`
- Existing source bootstrap contracts:
  - `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - `crates/source-bootstrap/tests/cli_contract.rs`
  - `crates/source-bootstrap/tests/fixtures/valid-source-bootstrap-config.yml`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and behavior priorities in this turn.
- This task is stricter than the earlier story-13 work:
  - task 01 proved the README should be sufficient
  - task 02 proved the Docker path should use direct container commands
  - task 03 must prove the actual config examples and quick-start steps are copyable, minimal, and directly useful
- The README contains two novice-facing config examples that matter here:
  - the source-bootstrap config under `## Source Bootstrap Quick Start`
  - the runner config under `## Docker Quick Start`
- If the first RED slice shows that a copyable README example cannot be tested through the real public CLI without inventing a markdown mini-framework or a fake config abstraction, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - each README config example is copyable and accepted by the real public CLI that owns it
  - the Docker quick start does not require the novice to infer missing filesystem layout or TLS-material setup
  - the quick start stays minimal enough for a novice to follow directly, not as a pseudo-reference document
  - later quick-start commands reuse the same config path, mapping id, and artifact paths introduced earlier
- Lower-priority concerns:
  - broader README prose polish outside the quick-start path
  - advanced multi-mapping examples in the quick-start itself

## Problem To Fix

- The source-bootstrap README YAML is not obviously copyable today:
  - `crates/source-bootstrap/src/config/parser.rs` requires `webhook.ca_cert_path`
  - the README source-bootstrap example currently omits that field
- The runner README quick start is not novice-minimal today:
  - it starts with a two-mapping config instead of one clean tracer-bullet mapping
  - it tells the operator to create TLS material but does not yet own a copyable command path for doing so
- The same public runner config contract is duplicated across multiple places:
  - README YAML
  - `mounted_config_text()` in `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/fixtures/valid-runner-config.yml`
  - `crates/runner/tests/fixtures/container-runner-config.yml`
- Current README tests are still too phrase-oriented to prove “works as written” for the actual config examples.

## Boundary And Interface Decisions

- Keep product CLI interfaces unchanged unless a RED slice exposes a real public-surface bug.
- Add one honest owner for each README copyable config example instead of duplicating sample YAML across prose, fixtures, and inline Rust strings.
  - preferred fixtures:
    - `crates/source-bootstrap/tests/fixtures/readme-source-bootstrap-config.yml`
    - `crates/runner/tests/fixtures/readme-runner-config.yml`
- Expand the README contract support boundary so tests can:
  - extract named YAML and bash blocks from the quick-start sections
  - compare the extracted sample config to the canonical README fixture after normalization
  - assert quick-start step order and identifier/path reuse without scattering string offsets everywhere
- Do not add a generic markdown parsing framework.
- Do not keep both inline sample strings and fixture files if they describe the same public contract. Delete or collapse the weaker duplicate.

## Public Contract To Establish

- The source-bootstrap README example is copyable:
  - when written with the documented companion CA file, `source-bootstrap render-bootstrap-script --config <path>` succeeds
- The Docker quick-start README example is copyable:
  - when written to a temp config path, `runner validate-config --config <path>` succeeds
  - the starting config is novice-minimal and should describe one mapping, not a multi-mapping reference bundle
- The Docker quick start explicitly owns the config directory and TLS-material setup before `validate-config` relies on those paths.
- The Docker quick start keeps one consistent mapping id, config path, cert path convention, and schema artifact path convention across later commands.
- The task fails if the README requires “you know what TLS files to create” or “you know which mapping id to substitute here” inference.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - sample-config knowledge is duplicated across README prose, fixture YAML, and one-off test helpers
- Required cleanup during execution:
  - make the README sample fixtures the honest owners of the copyable examples
  - move README code-block extraction and ordered-step assertions behind the existing README support boundary instead of adding more ad hoc `.contains(...)` calls
  - remove or collapse any inline config helper that only duplicates a fixture with one real caller
- If a new helper exists only to hide a tiny local transformation from its sole caller, inline it again. The goal is one honest owner, not another fake layer.

## Files And Structure To Add Or Change

- [x] `README.md`
  - add the missing copyable details for the public quick start
  - likely reduce the runner starting config to one mapping
  - likely add explicit TLS-material creation commands
  - likely fix the source-bootstrap example to include the required CA path contract
- [x] `crates/runner/tests/readme_contract.rs`
  - extend from phrase checks into executable copyability assertions
- [x] `crates/runner/tests/support/readme_contract.rs`
  - teach it to extract quick-start code blocks and enforce ordered quick-start contracts
- [x] `crates/runner/tests/config_contract.rs`
  - replace overlapping inline sample config with the honest public fixture where appropriate
- [x] `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - cover the README source-bootstrap sample as a real public config path
- [x] `crates/source-bootstrap/tests/cli_contract.rs`
  - only if a README-documented source-bootstrap surface still lacks an honest public assertion
- [x] `crates/runner/tests/fixtures/readme-runner-config.yml`
  - preferred canonical fixture for the README runner sample
- [x] `crates/source-bootstrap/tests/fixtures/readme-source-bootstrap-config.yml`
  - preferred canonical fixture for the README source-bootstrap sample
- [x] `crates/runner/tests/fixtures/valid-runner-config.yml`
  - only if it should be reduced or merged into the README sample fixture
- [x] `crates/runner/tests/fixtures/container-runner-config.yml`
  - keep only if its Docker-network-specific differences are real and unavoidable

## Vertical TDD Slices

### Slice 1: Tracer Bullet For The Source-Bootstrap README Config

- [x] RED: add one failing contract that extracts the README source-bootstrap YAML, writes it with the documented CA file companion, and runs `source-bootstrap render-bootstrap-script --config <path>`
- [x] GREEN: make the smallest README or fixture change needed to make the public example succeed
- [x] REFACTOR: move README config-block extraction into the README support boundary instead of leaving parsing logic inside the test body

### Slice 2: Tracer Bullet For The Runner README Config

- [x] RED: add one failing contract that extracts the README runner YAML, writes it to a temp file, runs `runner validate-config --config <path>`, and asserts the sample is the novice-minimal starting shape
- [x] GREEN: make the smallest README or fixture change needed to make the sample valid and minimal
- [x] REFACTOR: replace overlapping inline runner sample text with one honest fixture owner if the call graph shows the duplication is fake

### Slice 3: Prove The Quick Start Owns TLS Material And Filesystem Setup

- [x] RED: add a failing contract that requires the Docker quick start to create the config directory, create the TLS file locations it references, and do so before `validate-config`
- [x] GREEN: add only the minimum copyable quick-start commands needed to close the gap
- [x] REFACTOR: centralize quick-start step-order and code-block lookup rules inside the README support boundary

### Slice 4: Prove The Quick Start Reuses One Consistent Operator Vocabulary

- [x] RED: add the next failing contract that rejects hidden substitutions across the quick start such as undocumented mapping-id changes, path changes, or artifact-name changes
- [x] GREEN: tighten the README so later commands reuse the identifiers and paths introduced by the starting example
- [x] REFACTOR: keep identifier/path reuse checks in one support owner instead of duplicating raw strings across tests

### Slice 5: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so README sample-config knowledge has one honest owner and no fake helper layer remains

## TDD Guardrails For Execution

- Every new assertion must fail before the supporting README, fixture, or test-support change is added.
- Do not satisfy this task by saying the README sample is “illustrative only.” It must be copyable.
- Do not satisfy this task by broadening the README into a long reference section. Keep the quick start short and direct.
- Do not satisfy this task by telling the operator to inspect source code, tests, or repo fixtures for missing config details.
- Do not keep two different canonical novice samples for the same public contract without an explicit public reason.
- Do not swallow config-parse, file-read, or quick-start contract failures. They are the task.

## Boundary Review Checklist

- [x] One honest owner exists for the README source-bootstrap sample
- [x] One honest owner exists for the README runner sample
- [x] README code-block extraction and quick-start-order checks live behind one support boundary
- [x] The runner quick-start sample is novice-minimal instead of reference-heavy
- [x] TLS-material setup is documented explicitly, not implied
- [x] No error path is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
