# Plan: Verify The Registry-Only Novice User Flow From Published Images Alone

## References

- Task:
  - `.ralph/tasks/story-25-verify-novice-user-registry-only/01-task-verify-registry-only-novice-user-can-complete-the-supported-flow.md`
- Related earlier product-shape plans:
  - `.ralph/tasks/story-16-runtime-split-removals/04-task-remove-novice-user-dependence-on-repo-clone-and-local-tooling_plans/2026-04-19-published-images-only-novice-path-plan.md`
  - `.ralph/tasks/story-21-github-workflows-image-publish/04-task-publish-separate-docker-compose-artifacts-for-runner-verify-and-sql-images_plans/2026-04-20-compose-artifacts-plan.md`
- Current operator-facing contract:
  - `README.md`
  - `artifacts/compose/setup-sql.compose.yml`
  - `artifacts/compose/runner.compose.yml`
  - `artifacts/compose/verify.compose.yml`
- Current image and runtime contract coverage:
  - `crates/setup-sql/tests/bootstrap_contract.rs`
  - `crates/setup-sql/tests/image_contract.rs`
  - `crates/setup-sql/tests/support/source_bootstrap_image_harness.rs`
  - `crates/runner/tests/image_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/support/runner_image_artifact_harness.rs`
  - `crates/runner/tests/support/verify_image_artifact_harness.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public interface direction and highest-priority behaviors to verify.
- This turn started with no existing story-25 plan file, so the first deliverable is a separate plan artifact plus a task-file plan pointer.
- The supported novice-user surface is the published-image contract, not the repository:
  - `setup-sql`
  - `runner`
  - `verify`
- The honest verification target is a temporary operator workspace outside the repository root.
  - It may contain only copied config files, copied README examples, copied published Compose YAML, Docker itself, and pulled images.
  - It must not rely on `cargo`, `make`, `git`, `docker build`, or repo-relative file paths as part of the asserted novice flow.
- Test code may still build local stand-in images to simulate published refs during repository validation, but the public behavior under test must execute only through the shipped Docker and Docker Compose entrypoints.
- If the first RED slice proves one of the published Compose contracts cannot be exercised honestly from a temp workspace without repo-path leakage, hidden test-only bypasses, or a broader runtime redesign, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.
- If execution uncovers a real product or documentation defect, that is not a reason to weaken the contract:
  - create a bug immediately with the `add-bug` skill
  - ask for a task switch
  - do not mark this task passed

## Current State Summary

- `README.md` already claims the novice path starts from pulling published images only and does not require a repository checkout.
- The README also embeds three copyable Compose examples:
  - `setup-sql.compose.yml`
  - `runner.compose.yml`
  - `verify.compose.yml`
- Source-controlled Compose artifacts already exist under `artifacts/compose/`.
- Current tests verify important but narrower contracts:
  - each image has the right entrypoint and minimal runtime shape
  - JSON operator logging works on the public commands
  - setup-sql can emit SQL from mounted config
  - runner can validate config and start against a local PostgreSQL harness
  - verify can validate config and expose its public entrypoint
- What is still missing is one end-to-end novice-user contract that proves the supported actions can be completed from a repo-free operator workspace using only published-image surfaces.
- The main boundary smell from `improve-code-boundaries` is clear:
  - novice-user evidence is currently scattered across image-specific harnesses and README prose
  - each harness owns a fragment of the public journey
  - none of them owns the actual registry-only operator workspace as one typed contract
- If execution simply adds more one-off assertions into existing image tests, the repo will gain three or four overlapping definitions of what the novice path means.

## Boundary Decision

- Add one explicit registry-only novice verification boundary in the test suite.
  - Preferred location:
    - `crates/runner/tests/novice_registry_only_contract.rs`
    - `crates/runner/tests/support/novice_registry_only_harness.rs`
- This boundary should own the operator-workspace model:
  - create a temp directory outside the repo
  - materialize only the copied files a novice operator would actually have
  - inject image coordinates through environment variables
  - run only public `docker` and `docker compose` commands
  - capture stdout, stderr, exit status, and any basic runtime evidence needed by the contract
- Keep existing image artifact harnesses focused on what they already do well:
  - build local stand-in images
  - inspect entrypoints
  - provide a real image tag for tests
- Do not spread temp-workspace creation, Compose invocation, and registry-like image substitution across:
  - `source_bootstrap_image_harness.rs`
  - `runner_image_artifact_harness.rs`
  - `verify_image_artifact_harness.rs`
- One support owner should translate local test-built image tags into the env vars the README and Compose contracts already use.
- The novice contract test file should describe behavior only:
  - setup-sql README and Compose usage works without repo access
  - runner README and Compose usage works without repo access
  - verify Compose usage is usable without repo access
  - README/contributor leakage fails loudly

## Public Contract To Establish

- One contract fails if the novice flow requires any repository checkout, repo-relative path, or contributor-only document.
- One contract fails if any supported novice step requires `docker build` instead of a published image reference.
- One contract fails if the README quick-start commands cannot be reproduced from a temp operator workspace with only copied config files.
- One contract fails if the shipped Compose artifacts cannot be used from that same temp workspace with only the published image env vars set.
- One contract fails if the README or Compose path implicitly depends on hidden repository knowledge such as:
  - `crates/`
  - `tests/`
  - `investigations/`
  - `CONTRIBUTING.md`
  - AGENTS or contributor rules
- One contract fails if the registry-only flow stops covering the supported actions:
  - emit Cockroach setup SQL from `setup-sql`
  - emit PostgreSQL grants SQL from `setup-sql`
  - validate runner config from `runner`
  - start the runner from `runner` through its published interface
  - use the dedicated verify Compose contract without repo checkout
- `<passes>true</passes>` remains forbidden unless the verification completes without any new bug task.

## Improve-Code-Boundaries Focus

- Primary smell:
  - the novice-user contract is currently an emergent property of separate image tests, not one owned public boundary
- Required cleanup during execution:
  - create one honest support owner for temp-workspace setup and public Docker/Compose execution
  - keep image-build concerns in the existing artifact harnesses
  - keep behavior assertions in one new contract test rather than sprinkling them across existing image tests
- Secondary smell:
  - image-coordinate/env-var knowledge can drift across README examples, Compose artifacts, and tests
  - the new support owner should reuse the actual public env var names:
    - `SETUP_SQL_IMAGE`
    - `RUNNER_IMAGE`
    - `VERIFY_IMAGE`
- Tertiary smell:
  - tests that validate the novice path must not smuggle repo access through fixture paths mounted directly from the repository once the operator-workspace boundary exists
- If the support module turns into a pile of tiny wrappers around unrelated shell calls, flatten it again. The goal is one real owner, not another fake helper layer.

## Files And Structure To Add Or Change

- [ ] `.ralph/tasks/story-25-verify-novice-user-registry-only/01-task-verify-registry-only-novice-user-can-complete-the-supported-flow.md`
  - keep the plan pointer updated
- [ ] `crates/runner/tests/novice_registry_only_contract.rs`
  - new behavior-level contract for the registry-only novice path
- [ ] `crates/runner/tests/support/novice_registry_only_harness.rs`
  - preferred temp-workspace and public-command owner
- [ ] `crates/runner/tests/support/mod.rs`
  - export the new support module if needed
- [ ] `crates/runner/tests/support/runner_image_artifact_harness.rs`
  - only if execution needs a small extension to reuse built images under env-driven published refs
- [ ] `crates/runner/tests/support/verify_image_artifact_harness.rs`
  - same rule as above
- [ ] `crates/setup-sql/tests/support/source_bootstrap_image_harness.rs`
  - only if execution needs a minimal hook to expose the built setup-sql image tag to the new cross-image harness
- [ ] `README.md`
  - only if a RED slice exposes a real drift or repo-leaking operator step
- [ ] `artifacts/compose/*.compose.yml`
  - only if a RED slice proves a published Compose contract is not honestly repo-free

## TDD Approval And Behavior Priorities

- Highest-priority behaviors to prove:
  - a temp operator workspace can use the setup-sql README and Compose examples without repo access
  - a temp operator workspace can validate and start the runner through the published image interfaces only
  - the dedicated verify Compose contract remains a repo-free operator contract
  - the contract fails loudly if contributor-only or local-build assumptions re-enter the supported novice path
- Lower-priority concerns:
  - broader docs polish beyond the registry-only path
  - adding new public CLI switches
  - expanding into the full story-24 README-only e2e migration path

## Vertical TDD Slices

### Slice 1: Tracer Bullet For A Real Repo-Free Setup-SQL Workspace

- RED:
  - add one failing novice-path contract that creates a temp operator workspace, copies only the minimal README-style `config/` files, points `SETUP_SQL_IMAGE` at a locally built stand-in tag, and runs the shipped `setup-sql.compose.yml` through `docker compose`
  - assert:
    - the SQL artifact is emitted successfully
    - the command path uses the published-image env var, not `docker build`
    - no repo-relative file path is required by the workspace contract
- GREEN:
  - add the minimum novice harness needed to materialize the temp workspace and run the public Compose command
- REFACTOR:
  - keep image-build knowledge outside the new novice harness

### Slice 2: README Runner Validation And Runtime Startup From The Same Workspace Model

- RED:
  - add the next failing contract for the runner README path:
    - `docker run ... validate-config`
    - `docker run ... run`
  - assert:
    - validation works from copied config and certs only
    - runtime starts through the public image interface
    - health can be observed without consulting repo docs or local build steps
- GREEN:
  - extend the novice harness with the smallest support needed for runner temp-workspace startup
- REFACTOR:
  - reuse the existing typed runner runtime helpers where they stay honest, but keep repo-mounted fixture shortcuts out of the novice path

### Slice 3: Dedicated Compose Contracts Stay Repo-Free

- RED:
  - add failing assertions that the shipped `artifacts/compose/*.compose.yml` files can be copied into the temp operator workspace and executed with only image env vars plus operator-managed config files
  - cover at minimum:
    - `setup-sql.compose.yml`
    - `runner.compose.yml`
    - `verify.compose.yml`
  - assert these contracts do not depend on:
    - local build contexts
    - repo paths
    - contributor docs
- GREEN:
  - make the smallest artifact or harness changes needed so the copied Compose contracts work honestly from the temp workspace
- REFACTOR:
  - keep Compose-file copying and env injection centralized in the novice harness

### Slice 4: Contributor Leakage And Local-Build Regressions Fail Loudly

- RED:
  - add failing assertions that reject novice-path leakage such as:
    - `docker build`
    - `cargo run`
    - repo checkout instructions
    - references to `CONTRIBUTING.md`, `AGENTS.md`, `crates/`, or `tests/`
  - apply those assertions to both README-driven commands and the copied Compose workflow
- GREEN:
  - remove or correct only the real leaking surface that the RED slice finds
- REFACTOR:
  - keep the forbidden-path and forbidden-command rules in one typed place rather than repeating literal string checks everywhere

### Slice 5: Bug-Handoff Rule And Final Verification Lanes

- RED:
  - execute the required repository lanes one at a time:
    - `make check`
    - `make lint`
    - `make test`
  - if any verification step reveals a real product/doc defect, stop normal execution immediately, create a bug with `add-bug`, ask for a task switch, and keep `<passes>false</passes>`
- GREEN:
  - continue until all required default lanes pass with the new novice-path contract in place
- REFACTOR:
  - do one final `improve-code-boundaries` pass so the registry-only novice path has one honest owner and does not regress into scattered per-image assertions

## Guardrails For Execution

- Every new assertion must fail before the supporting change is added.
- Do not satisfy this task with README text grep alone. The public behavior must execute through real Docker and Docker Compose commands in a temp operator workspace.
- Do not use `docker build` as part of the asserted novice flow, even if local image builds remain necessary behind the scenes to stand in for published refs during repository tests.
- Do not let the new novice harness mount repository fixture directories directly as the asserted operator workspace.
- Do not expand this task into the full zero-repo migration and verify-API e2e story if the stricter registry-only contract can be proven honestly with smaller public-surface checks.
- Do not swallow discovered defects.
  - If a real bug appears, create an `add-bug` task immediately and ask for the task switch instead of weakening the assertion.
- Do not mark this task passed if any bug task is created during verification.

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] Do not run `make test-long` unless execution changes ultra-long test selection or the task explicitly proves the long lane is required
- [ ] One final `improve-code-boundaries` pass after the required lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only if no new bug task was required

Plan path: `.ralph/tasks/story-25-verify-novice-user-registry-only/01-task-verify-registry-only-novice-user-can-complete-the-supported-flow_plans/2026-04-20-registry-only-novice-flow-plan.md`

NOW EXECUTE
