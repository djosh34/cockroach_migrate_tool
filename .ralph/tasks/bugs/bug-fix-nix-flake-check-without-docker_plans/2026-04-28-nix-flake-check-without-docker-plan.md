# Plan: Fix `nix flake check` Without Docker Test Assumptions

## References

- Task:
  - `.ralph/tasks/bugs/bug-fix-nix-flake-check-without-docker.md`
- Current flake and command surfaces:
  - `flake.nix`
  - `Makefile`
- Long-lane and verify runtime surfaces:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/verify_service_harness.rs`
- Current runtime-facing docs and artifacts that may need truth-fixing if they imply a test path:
  - `README.md`
  - `artifacts/compose/runner.compose.yml`
  - `artifacts/compose/verify.compose.yml`
- Adjacent completed migration tasks that set the current architectural direction:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only.md`
- Adjacent planned task that must not be accidentally solved here:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/05-task-enable-development-without-host-nix.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/05-task-enable-development-without-host-nix_plans/2026-04-28-enable-development-without-host-nix-plan.md`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn started with no bug-specific plan artifact, so this turn is planning-only and must stop after the plan is written.
- The execution turn must stay Nix-native end to end.
  - Do not run Docker, Docker Compose, container runtimes, or image-based test flows as part of reproducing or fixing this bug.
  - Do not reintroduce Cargo as an execution path; use Nix commands and flake outputs.
- TDD still applies here, but the public contract is executable Nix behavior rather than file-content assertions.
  - RED for this task means a real `nix build`, `nix run`, `make`, or equivalent flake-backed command fails honestly.
  - GREEN means the smallest code or flake change makes that one public command pass.
- The acceptance criteria require the final execution turn to prove:
  - every failing `nix flake check` component was identified
  - the verify-binary failure path is fixed without Docker assumptions
  - the long-lane failure path is fixed without Docker assumptions
  - stale code, tests, docs, or naming that still imply Docker-backed tests are removed or rewritten
- If execution proves the current long-lane failures are caused by a broader product defect unrelated to Nix packaging or Docker assumptions, this plan must switch back to `TO BE VERIFIED` instead of smuggling a second task into this bug.

## Current State Summary

- `flake.nix` currently exposes these relevant public check surfaces:
  - `runner-crate`
  - `runner-crate-clippy`
  - `runner-crate-fmt`
  - `runner-crate-nextest`
  - `runner-crate-nextest-long`
  - `molt-go-test`
- `runner-crate-nextest-long` already starts the real long lane without Docker.
  - `crates/runner/tests/support/e2e_harness.rs` shows:
    - `create_network()` is a no-op
    - CockroachDB starts as a local process from `COCKROACH_BIN`
    - PostgreSQL starts as a local process from `postgres`
    - the runner is a host process
    - verify correctness is exercised through a real `molt verify-service` host process
- That means the long-lane problem is not "port the harness off Docker."
  - The likely bug is that the flake check graph, dependency declaration, or test naming still assumes an older image/container surface.
- `verify-binary` is currently both:
  - a package output in `packages`
  - a native input for the long lane
- There is also already a separate Go test surface:
  - `molt-go-test`
- That split is suspicious in exactly the wrong-place way this bug should attack:
  - packaging and Go test execution may still be coupled when they should be separate flake boundaries
- The repo still has public runtime-image docs and compose artifacts in `README.md` and `artifacts/compose/`.
  - Those are not automatically wrong, because they may describe runtime usage rather than tests.
  - But execution must remove or rewrite any wording that falsely implies they are part of the flake-check or long-test validation path.
- Execution findings from the current pass:
  - `runner-crate-fmt` is green again.
  - `molt-go-test` is green after giving the explicit Go test derivation its own Nix-native Postgres and Cockroach runtime plus the repo fixture certs it actually consumes.
  - `runner-crate-nextest-long` no longer fails on the removed `enriched` envelope value after shifting the harness toward wrapped webhook payloads with `full_table_name`.
  - The current focused reproduction is now explicit:
    - `nix build -L .#checks.${system}.test-long` fails in the ignored long-lane suite at `audited cockroach sql failed`
    - Cockroach returns `ERROR: use of CHANGEFEED requires an enterprise license` with `SQLSTATE: XXC02`
    - the failure happens before webhook delivery or payload processing, so the broken boundary is the supplied Cockroach runtime itself
  - The Nix flake currently pulls `cockroachdb 23.1.14`, while the code only asserts a `23.1.` prefix.
  - Current Cockroach Labs licensing docs say the modern "no key required for single-node developer clusters" behavior applies to `v23.1.29`, `v23.2.15`, `v24.1.7`, `v24.2.5`, `v24.3.0`, and later.
  - That means the current execution blocker is not "CDC must be redesigned".
    - It is a wrong-place runtime-version boundary: the flake silently supplies an older `23.1.14` Cockroach binary whose licensing behavior does not match the current documented single-node development behavior.

## Chosen Execution Design

- Keep the real public CDC contract intact:
  - Cockroach source
  - sink-backed webhook changefeed
  - runner webhook ingest
  - helper persistence
  - verify-service reconciliation
- Do not replace the webhook-based long lane with a sinkless CDC workaround.
  - Sinkless `CREATE CHANGEFEED` is not a drop-in replacement for the runner webhook contract this suite exists to verify.
- Fix the runtime boundary directly:
  - introduce one explicit flake-owned `cockroachdb-runtime` derivation instead of relying on ambient `pkgs.cockroachdb`
  - pin that runtime to a post-transition `23.1` patch whose documented single-node behavior matches the intended local-dev test contract
  - target `23.1.30` if available; otherwise use the newest available `23.1.x` patch that is `>= 23.1.29`
- Route every Nix-native Cockroach test surface through that one runtime boundary:
  - `runner-crate-nextest-long`
  - `molt-go-test-harness`
  - `devShells.default` via `COCKROACH_BIN`
- Keep package/test ownership explicit while doing it:
  - `verify-binary` remains only the packaged Go runtime artifact
  - `molt-go-test` remains the explicit Go check derivation
- Abort back to `TO BE VERIFIED` only if the focused red test still fails with the upgraded `23.1.x` runtime, because that would prove the issue is broader than the currently verified version boundary.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - `flake.nix` currently mixes package ownership, Go-test ownership, long-lane ownership, and Cockroach runtime-version ownership in one muddy place.
  - The worst wrong-place detail is that long-lane semantics depend on whichever `pkgs.cockroachdb` happens to arrive from the locked nixpkgs revision, even though the suite actually needs a specific single-node CDC behavior contract.
- Desired boundary after execution:
  - package derivations build packages
  - explicit check derivations own test execution
  - one explicit `cockroachdb-runtime` derivation owns the Cockroach version contract for every Nix-backed test surface
  - long-lane derivations declare only the native host-process dependencies they truly need
  - tests and docs stop pretending there is a Docker-backed correctness lane
- Concrete smells to remove during execution:
  - `verify-binary` behaving like both a package and a hidden test runner
  - the Cockroach runtime contract being hidden inside ambient nixpkgs package selection instead of a named test/runtime boundary
  - one monolithic `runner-crate-nextest-long` failure surface that is too muddy to tell which long-lane contract is actually broken
  - leftover `container` or image-oriented naming in long-lane tests or support where the runtime is demonstrably a host process
  - docs or artifact names that tell contributors to think of compose/image usage as the validation path for this bug

## Intended Public Contract After Execution

- `nix flake check` passes on the current system without Docker, Docker Compose, or image-based tests.
- The verify build path is explicit and honest:
  - package builds do not silently own unstable DB-backed tests
  - Go tests, if still part of flake checks, run only from explicit test derivations with the dependencies they actually need
- The long lane is explicit and honest:
  - it uses only Nix-supplied host-process dependencies
  - its Cockroach runtime version is explicit rather than inherited accidentally from ambient nixpkgs state
  - the default bootstrap path can create its sink-backed changefeed on the chosen single-node `23.1.x` runtime
  - it no longer carries misleading Docker/container naming where that naming describes the test mechanism rather than the product runtime
- The repo no longer claims or implies that Docker-backed tests still exist.
  - Runtime image docs may remain only if they are clearly runtime/operator docs rather than test instructions.

## Files And Structures Expected To Change

- `flake.nix`
  - main fix site for the explicit Cockroach runtime derivation plus package/test ownership cleanup
- `crates/runner/tests/default_bootstrap_long_lane.rs`
  - add the focused red tracer-bullet test for successful default bootstrap through `CREATE CHANGEFEED`
- `crates/runner/tests/support/e2e_harness.rs`
  - only if the long-lane bootstrap boundary needs a smaller public helper or truth-fixing around runtime naming
- `crates/runner/tests/support/verify_service_harness.rs`
  - only if the verify runtime contract or binary lookup is misdeclared for Nix-native execution
- `README.md`
  - only for wording cleanup if it still implies Docker-backed test execution
- `artifacts/compose/runner.compose.yml`
- `artifacts/compose/verify.compose.yml`
  - only if their names or referenced docs falsely present them as a validation path rather than runtime examples

## Type And Interface Decisions

- Do not add fake repo-string tests for Nix or README content.
  - For this task, real commands are the public interface.
- Prefer small, focused flake check derivations over one muddy "all ignored tests" reproduction loop.
  - If execution needs focused long-lane red/green iteration, add or temporarily route through an honest focused check attr instead of repeatedly guessing under the full ignored suite.
- Introduce one named Cockroach runtime boundary.
  - Long-lane and Go-harness code should depend on `cockroachdb-runtime`, not on an unspoken ambient `pkgs.cockroachdb` selection.
- Keep package and test ownership separate.
  - If `verify-binary` is only a runtime/package artifact, it should not be the place where wide Go test execution happens.
- Keep runtime docs separate from validation docs.
  - Do not delete runtime image examples merely because they use images.
  - Do delete or rewrite any wording that says or implies those image examples are how `nix flake check` or long-lane correctness is validated.

## TDD Execution Order

### Slice 0: Enumerate Every Failing Flake Check Component Without Guessing

- [ ] RED:
  - run the current flake check graph through focused Nix builds, not Docker:
    - enumerate the current system and the available `checks`
    - build each relevant check/package surface individually until every failing component is known
  - record exact failing components and keep the first real failure as the next tracer bullet
- [ ] GREEN:
  - none here except getting to a complete, honest failure inventory
- [ ] REFACTOR:
  - if enumeration itself is muddy because one aggregate derivation hides the real failing behavior, introduce a more focused derivation boundary before fixing behavior

### Slice 1: Focused Long-Lane Runtime-Version Tracer Bullet

- [ ] RED:
  - add one focused ignored integration test in the long-lane suite that proves default bootstrap succeeds far enough to create the initial sink-backed changefeed on the supplied local single-node runtime
  - run only that focused Nix-native long-lane surface and capture the current `XXC02` enterprise-license failure under `cockroachdb 23.1.14`
- [ ] GREEN:
  - introduce the explicit `cockroachdb-runtime` derivation at `23.1.30` or the newest available `23.1.x` patch that is `>= 23.1.29`
  - rewire the long-lane derivation, Go harness, and dev shell to use that runtime boundary instead of ambient `pkgs.cockroachdb`
  - make only the focused long-lane tracer bullet green first
- [ ] REFACTOR:
  - if both long-lane and Go harnesses need the same binary, collapse the version contract into one shared flake binding and remove duplicate/runtime-specific wiring
- Stop condition:
  - if the focused tracer bullet still fails after the runtime upgrade, switch the plan back to `TO BE VERIFIED`

### Slice 2: Revalidate Verify Boundary And Remaining Long-Lane Failures

- [ ] RED:
  - rerun the already-fixed explicit Go check and the next failing long-lane scenario after Slice 1 is green
  - if a new failure remains, take exactly one scenario as the next red slice
- [ ] GREEN:
  - keep `verify-binary` as a pure build artifact and `molt-go-test` as the explicit Go check
  - fix only the next real failing scenario, one at a time, under the upgraded runtime boundary
- [ ] REFACTOR:
  - keep the check names honest so future failures tell us whether the runtime, webhook ingest, or verify path regressed

### Slice 3: Remove Stale Docker/Test Vocabulary And Wrong-Place Naming

- [ ] RED:
  - identify the first still-shipped code/doc/name that falsely implies Docker-backed tests are part of this repo’s correctness path
- [ ] GREEN:
  - rewrite or delete that stale surface
  - likely hotspots:
    - long-lane test names mentioning destination containers where the runtime is a host process
    - docs that frame compose artifacts as validation instead of runtime usage
- [ ] REFACTOR:
  - prefer one truthful term consistently:
    - `host process`
    - `local process`
    - `runtime image`
    - `runtime example`
  - avoid mixing those with old "test container" language

### Slice 4: Finish The Remaining Failing Flake Components One By One

- [ ] RED:
  - after each green slice, rerun the next failing flake check component
  - if the bug still holds, take the next failing component as a new red slice
- [ ] GREEN:
  - keep fixing one failing behavior at a time until:
    - verify-related failures are gone
    - long-lane failures are gone
    - no Docker-backed test assumptions remain
- [ ] REFACTOR:
  - after the last focused fix, do one deliberate `improve-code-boundaries` pass over flake/test naming ownership

### Slice 5: Final Required Validation

- [ ] `nix flake check`
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long`
- [ ] one final `improve-code-boundaries` review after every lane is green

## Execution Guardrails

- Do not run Docker, Docker Compose, or any image-based test path during execution.
- Do not use Cargo directly; stay inside Nix-backed commands and derivations.
- Do not "fix" this by disabling the long lane wholesale.
- Do not silently drop Go tests just to make the flake green.
  - if a Go test is unstable because it belongs to a different boundary, move it to the right explicit boundary and keep coverage honest
- Do not delete runtime image docs merely because they mention images.
  - only remove or rewrite them if they incorrectly describe a test or validation surface
- Do not swallow any failing command output; every failure must either be fixed in this task or turned into a separate bug if it is truly out of scope

## Final Verification Checklist For The Execution Turn

- [ ] Every failing `nix flake check` component was explicitly identified
- [ ] The first verify-related bug was fixed through a real red/green executable step
- [ ] The first long-lane bug was fixed through a real red/green executable step
- [ ] No Docker or image-based test assumption remains in code, docs, workflows, or naming that describe the validation path
- [ ] `nix flake check` passes cleanly
- [ ] `make check` passes cleanly
- [ ] `make lint` passes cleanly
- [ ] `make test` passes cleanly
- [ ] `make test-long` passes cleanly because this bug explicitly owns long-lane failures
- [ ] One final `improve-code-boundaries` pass confirms package/test/runtime ownership is cleaner than before

Plan path: `.ralph/tasks/bugs/bug-fix-nix-flake-check-without-docker_plans/2026-04-28-nix-flake-check-without-docker-plan.md`

NOW EXECUTE
