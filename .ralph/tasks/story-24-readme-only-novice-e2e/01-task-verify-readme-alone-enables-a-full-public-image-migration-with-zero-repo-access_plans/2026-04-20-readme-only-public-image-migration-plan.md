# Plan: Verify README-Only Public-Image Migration With Zero Repo Access

## References

- Task:
  - `.ralph/tasks/story-24-readme-only-novice-e2e/01-task-verify-readme-alone-enables-a-full-public-image-migration-with-zero-repo-access.md`
- Current operator-facing contract:
  - `README.md`
  - `artifacts/compose/setup-sql.compose.yml`
  - `artifacts/compose/runner.compose.yml`
  - `artifacts/compose/verify.compose.yml`
- Existing novice/public-image coverage:
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/support/novice_registry_only_harness.rs`
  - `crates/runner/tests/support/published_image_refs.rs`
- Existing verify/public API coverage worth reusing:
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public interface direction and the highest-priority behaviors to verify.
- This turn started without a story-24 plan file, so the correct output is a separate plan artifact plus a task-file `<plan>` pointer.
- The product interface under test is stricter than story 25:
  - the user may read only `README.md`
  - the user may use only Docker plus pulled public images
  - the user must complete the ordered flow:
    - emit Cockroach SQL
    - apply that exact Cockroach SQL
    - emit PostgreSQL SQL
    - apply that exact PostgreSQL SQL
    - run the runner from a pulled image
    - run the verify HTTP API from a pulled image
- The asserted operator workspace must be a temp directory outside the repository root and must be assembled from README-fenced content plus copied public Compose artifacts only.
- Repository tests may still build local stand-in images to simulate published tags, but the public behavior under test must execute only through `docker run` and `docker compose`.
- If execution proves the README omits a required operator value, the public images do not support the documented mTLS path, or the runtime reports misleading auth/connectivity failures, that is a product bug:
  - create a bug immediately with `add-bug`
  - ask for a task switch
  - keep `<passes>false</passes>`

## Current State Summary

- `README.md` already claims the novice path is published-image-only and repo-free.
- The README already contains inline config snippets and the three Compose examples, but the current test boundary does not yet prove a user can complete the full independent migration by reading only those inline docs.
- `crates/runner/tests/support/novice_registry_only_harness.rs` is the main boundary smell:
  - it hardcodes operator config content inside the harness
  - it copies repo fixtures directly instead of deriving the operator workspace from README material
  - it mixes README-facing workspace assembly, runtime orchestration, Compose artifact copying, and verification-service startup in one file
- Existing coverage is narrower than the story:
  - registry-only setup-sql emission is covered
  - runner config validation and runtime startup are covered
  - verify-service image and HTTP job flow are covered separately
  - exact README-only SQL ordering, README-derived secure config values, and wrong-config failure clarity are not yet owned by one honest contract

## Boundary Decision

- Add a dedicated README-only public-image contract rather than overloading the existing registry-only contract.
  - Preferred test entrypoint:
    - `crates/runner/tests/readme_public_image_contract.rs`
- Split the support boundary so one module owns README-derived operator workspace assembly and one module owns runtime orchestration.
  - Preferred support files:
    - `crates/runner/tests/support/readme_public_image_workspace.rs`
    - `crates/runner/tests/support/readme_public_image_harness.rs`
- Keep `published_image_refs.rs` focused on stand-in image tag creation only.
- Reuse lower-level runtime helpers only where the public boundary stays honest.
  - Good reuse:
    - image-ref helpers
    - existing verify API polling/client patterns
    - existing secure source/destination test infrastructure from the e2e support
  - Forbidden reuse:
    - handwritten config content that can drift from the README
    - repo-mounted operator workspaces
    - hidden repo-path shortcuts in the asserted novice flow

## Intended Public-Test Shape

- `readme_public_image_workspace` should parse or otherwise extract only the README-owned fenced examples and commands needed for the flow.
  - It should materialize:
    - `config/cockroach-setup.yml`
    - `config/postgres-grants.yml`
    - `config/runner.yml`
    - `config/verify-service.yml`
    - README-required cert paths under `config/certs/`
    - copied Compose files
  - It should reject contributor-only leakage in the README surface:
    - repo checkout instructions
    - `git`
    - `docker build`
    - `cargo`
    - `make`
    - references to `crates/`, `tests/`, `investigations/`, `AGENTS.md`, or `CONTRIBUTING.md`
- `readme_public_image_harness` should own the end-to-end operator journey through public commands only.
  - It should:
    - inject stand-in published image refs via env vars
    - run `docker compose` and `docker run` from the temp workspace
    - apply the exact emitted SQL without mutation
    - start the runner and verify-service through their public container surfaces
    - exercise the verify HTTP API over HTTPS/mTLS
    - capture stderr/stdout/status for clear failure assertions

## Vertical TDD Slices

### Slice 1: Tracer Bullet For README-Derived Workspace Assembly

- RED:
  - add one failing contract that builds a temp operator workspace entirely from README-owned inline content plus copied Compose artifacts
  - assert the workspace can be materialized without touching repository fixtures as operator inputs
  - assert the README-owned surface forbids repo/build/contributor leakage
- GREEN:
  - add the smallest `readme_public_image_workspace` support needed to extract the required config snippets and commands
- REFACTOR:
  - centralize README block extraction and forbidden-marker checks in the workspace module so later tests do not duplicate stringly parsing logic

### Slice 2: Exact SQL Ordering And No-Rewrite Application

- RED:
  - add the next failing contract that runs the README-guided setup flow and proves the order is enforced:
    - emit Cockroach SQL first
    - apply exactly that emitted Cockroach SQL
    - emit PostgreSQL SQL second
    - apply exactly that emitted PostgreSQL SQL
  - assert the harness does not rewrite or template-edit the emitted SQL before application
- GREEN:
  - reuse or extend secure source/destination test infrastructure so the emitted SQL can be applied against real test databases with the README-owned workspace inputs
- REFACTOR:
  - keep SQL emission/application evidence in one typed result object instead of passing raw strings and temp paths around ad hoc

### Slice 3: README-Derived Secure mTLS Config Values Drive Runner Startup

- RED:
  - add the next failing contract that proves the required secure config values come from the README-owned workspace alone and that the runner starts from a pulled image with those values
  - assert validation and startup happen through the documented public commands only
  - add a failing wrong-config case that demonstrates operator-visible failure output for invalid auth material
- GREEN:
  - extend the harness with the minimum secure runtime setup needed to bring up the runner honestly from the README-derived workspace
- REFACTOR:
  - move secure config/cert materialization behind a small typed workspace API instead of scattering cert copy paths across the harness

### Slice 4: Authentication Failures Must Not Masquerade As Connectivity Failures

- RED:
  - add failing assertions for at least two distinct negative cases:
    - bad credentials or client-auth material
    - real connectivity failure
  - assert the error output is operator-usable and distinguishes auth failures from connection failures
- GREEN:
  - make the minimum product or documentation changes required so the surfaced errors match the real failure mode
- REFACTOR:
  - keep negative-case process launching and stderr capture in one helper instead of duplicating shell invocation logic
- Stop condition:
  - if this RED slice reveals a real product defect, create a bug task immediately and ask for a task switch instead of weakening the contract

### Slice 5: Verify HTTP API Works From The README-Only Workspace

- RED:
  - add the next failing contract that starts the published verify-service image from the README-derived workspace and exercises the HTTPS API through its public endpoints
  - assert the user can reach readiness and submit at least one verification job using only README-derived cert paths and config
- GREEN:
  - reuse the existing verify HTTP client/polling pattern where it stays public-surface honest
- REFACTOR:
  - keep verify-service runtime/client logic separate from workspace parsing so README extraction and API orchestration do not collapse back into one large harness

### Slice 6: Final Boundary Cleanup And Required Repository Lanes

- RED:
  - run the required repository lanes after the behavior slices are green:
    - `make check`
    - `make lint`
    - `make test`
  - if any lane or manual verification exposes a real product/doc defect, create a bug immediately, ask for a task switch, and do not mark the task passed
- GREEN:
  - continue until all required default lanes pass cleanly with the new README-only contract in place
- REFACTOR:
  - do one final `improve-code-boundaries` pass to ensure README workspace assembly, runtime orchestration, and image-ref ownership are cleanly separated

## Guardrails For Execution

- Every new behavior test must fail before the supporting change is added.
- Do not satisfy this task with README grep checks alone. The README content must drive a real Docker-based public workflow.
- Do not let the new README-only contract silently fall back to repo fixtures for operator inputs.
- Do not mutate emitted SQL before applying it; exact emitted SQL is the contract under test.
- Do not merge README parsing, operator workspace assembly, runner lifecycle control, and verify API orchestration back into one support file.
- Do not swallow defects.
  - If execution finds one, create an `add-bug` task immediately and ask for a task switch.
- Do not mark this task passed if any bug task is created during verification.
- Do not run `make test-long` unless execution changes ultra-long selection or the task explicitly proves the long lane is required.

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] Do not run `make test-long` unless the implementation changes ultra-long selection or the task explicitly requires it
- [ ] Final `improve-code-boundaries` pass confirms the README-only path has one honest owner per concern
- [ ] Update the task file checkboxes and set `<passes>true</passes>` only if no bug task was required

Plan path: `.ralph/tasks/story-24-readme-only-novice-e2e/01-task-verify-readme-alone-enables-a-full-public-image-migration-with-zero-repo-access_plans/2026-04-20-readme-only-public-image-migration-plan.md`

NOW EXECUTE
