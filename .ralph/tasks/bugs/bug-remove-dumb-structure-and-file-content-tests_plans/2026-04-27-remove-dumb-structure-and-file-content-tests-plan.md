# Plan: Remove Dumb Structure And File-Content Tests

## References

- Task: `.ralph/tasks/bugs/bug-remove-dumb-structure-and-file-content-tests.md`
- Representative dumb-test surfaces already identified:
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/tls_reference_contract.rs`
  - `crates/runner/tests/e2e_integrity_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/verify_source_contract.rs`
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/verify_source_contract.rs`
  - `crates/runner/tests/support/tls_reference_surface.rs`
  - `crates/runner/tests/support/readme_operator_surface.rs`
  - `crates/runner/tests/support/readme_operator_workspace.rs`
  - `crates/runner/tests/support/e2e_integrity_contract_support.rs`
  - `crates/runner/tests/support/verify_docker_contract.rs`
  - `crates/setup-sql/tests/image_contract.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because the active bug task has no existing execution plan marker.
- The task description is the approval for a planning pass and already defines the acceptance criteria.
- No backwards compatibility is required.
  - If a current test module exists only to preserve file layout, helper names, docs phrasing, YAML shape, or Dockerfile wording, execution should delete it instead of preserving it.
- `make test-long` remains out of scope unless the cleanup changes long-lane support or the only honest replacement for a removed dumb test is a long-lane behavioral check.
- Required execution lanes remain:
  - `make check`
  - `make lint`
  - `make test`
- If the first execution slices prove that a suspected dumb test is actually the only honest coverage for a real supported behavior, the plan must switch back to `TO BE VERIFIED` before more code moves.

## Current State Summary

- The repo currently contains multiple test modules whose primary behavior is reading adjacent repo files and asserting text or structure.
- The main offender families are:
  - README and TLS doc scanners
    - `readme_operator_surface_contract.rs`
    - `tls_reference_contract.rs`
    - support modules that load and parse `README.md` or `docs/tls-configuration.md`
  - Dockerfile and image layout scanners
    - `runner_docker_contract.rs`
    - `verify_docker_contract.rs`
    - tests that assert `FROM scratch`, `ARG TARGETARCH`, fixed copy commands, or exact runtime file lists by inspecting source files
  - verify-slice tree scanners
    - `verify_source_contract.rs`
    - tests that assert top-level file lists, absent modules, or forbidden imports by scanning vendored Go source
  - E2E integrity/source-structure scanners
    - `e2e_integrity_contract.rs`
    - support that reads test files and asserts helper names, method names, or forbidden call sites
  - compose/YAML shape scanners
    - `novice_registry_only_contract.rs` tests that deserialize copied compose files and pin internal mount or network declarations instead of validating the runtime behavior through Compose itself
- This is a boundary problem, not just a bad-assertion problem.
  - Test code in `support/` has become a second owner of repo structure, docs wording, Dockerfile internals, and helper placement.
  - Those support modules create fake public contracts around implementation layout that the product does not actually expose.

## Improve-Code-Boundaries Focus

- Primary smell: wrong-place test ownership.
  - Documentation wording belongs to docs review, not Rust tests.
  - Dockerfile layer ordering belongs to image build/runtime behavior, not Rust string scanning.
  - Compose wiring belongs to Compose execution behavior, not YAML-field assertions.
  - Internal source-tree trimming belongs to the actual shipped image/command surface, not repo-file allowlists.
- Preferred cleanup direction:
  - delete test modules and support modules that only read adjacent files
  - keep behavioral harnesses that build images, run commands, start runtimes, poll APIs, or otherwise cross a real product boundary
  - if a support module mixes real runtime helpers with file scanners, split or delete the scanner owner so each remaining helper owns one real boundary
- Bold refactor allowance:
  - remove entire support files or whole test files if they only exist for structure/text assertions
  - collapse duplicated helper layers when a helper has only one caller and exists only to shuttle repo text into assertions

## Public Contract After Execution

- The test suite should verify supported behavior through real product boundaries only.
- Unsupported contracts after cleanup:
  - README wording, section order, word counts, inline fenced-block snippets
  - TLS reference doc phrasing or cross-link text
  - Dockerfile source text, cache-layer wording, or exact source-stage commands
  - compose YAML field layout, mount list shape, or network stanza shape
  - vendored Go source tree shape, file allowlists, forbidden helper names, or import-marker scans
  - Rust test/helper call-site markers, helper names, or source-file organization
- Supported behavior that may remain covered:
  - images build successfully
  - image entrypoints and CLI surfaces behave correctly when invoked
  - runtime APIs start and answer correctly
  - published compose flows run and exhibit the supported operator behavior
  - verify and runner behavioral checks continue to work through their real command/runtime surfaces

## Files And Structure To Change

- Delete or heavily reduce dumb-test owners:
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/tls_reference_contract.rs`
  - `crates/runner/tests/e2e_integrity_contract.rs`
  - `crates/runner/tests/verify_source_contract.rs`
- Delete support modules that exist only for adjacent-file scanning:
  - `crates/runner/tests/support/tls_reference_surface.rs`
  - `crates/runner/tests/support/readme_operator_surface.rs`
  - `crates/runner/tests/support/readme_operator_workspace.rs`
  - `crates/runner/tests/support/e2e_integrity_contract_support.rs`
  - `crates/runner/tests/support/verify_source_contract.rs`
- Reduce Dockerfile/compose scanner helpers to real runtime helpers only:
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/verify_docker_contract.rs`
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/setup-sql/tests/image_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
- Audit the whole tracked test tree for similar patterns beyond the identified list.
  - Search for `fs::read_to_string(...)` of repo artifacts
  - Search for tests that assert `contains`, `!contains`, helper names, headings, path allowlists, or source markers against adjacent files
  - Remove matching tests even if they are not named in the bug description

## Test Strategy

- Follow `tdd` only when a removed dumb test was protecting real behavior and the behavioral gap is not already covered elsewhere.
- Use strict vertical slices.
  - one failing behavioral test
  - smallest code or test-harness change to make that one slice green
  - refactor only after green
- Do not invent replacement tests for docs, Dockerfiles, workflows, compose shape, or source-tree shape.
- Prefer these honest replacements when needed:
  - image behavior: `docker build`, inspect entrypoint by running the built image, and exercise supported commands
  - compose behavior: run the compose workflow from a copied workspace and assert the runtime outcome
  - CLI behavior: invoke the command and assert observable stdout/stderr/exit status
  - HTTP/runtime behavior: start the service and make real requests

## TDD Execution Order

### Slice 1: Repository-Wide Dumb-Test Audit

- [ ] Audit the full tracked test suite for adjacent-file scanners and list every file that should be removed or reduced
- [ ] Identify which suspected tests already have honest behavioral coverage elsewhere so execution can delete them without replacement
- [ ] If any suspect test appears to guard a real behavior with no existing behavioral coverage, note that gap before deleting anything

### Slice 2: Delete Pure README/TLS/Text Contract Tests

- [ ] Remove README wording/order/word-count tests and their support loaders
- [ ] Remove TLS reference documentation wording/link-shape tests and their support loaders
- [ ] Confirm no remaining test scans docs or README for headings, snippets, or link text

### Slice 3: Delete Pure Source-Structure And Integrity Contract Tests

- [ ] Remove `verify_source_contract.rs` and its source-tree scanning support if nothing behavioral depends on it
- [ ] Remove `e2e_integrity_contract.rs` and its support if it only scans test/helper source files
- [ ] Inline or delete any single-caller helper created only to support source scanning

### Slice 4: Reduce Dockerfile And Compose Scanners To Real Behavior

- [ ] Delete Dockerfile text assertions from runner, verify, and setup-sql image tests
- [ ] Delete compose YAML shape assertions that pin mounts, configs, or network fields by reading artifact text
- [ ] Keep only image/compose tests that build, run, or exercise the shipped runtime behavior
- [ ] RED: if deleting a scanner exposes a real behavioral hole, add one failing behavioral test through Docker or Compose before continuing
- [ ] GREEN: implement only the minimum harness or product change required to satisfy that one behavioral test
- [ ] REFACTOR: remove duplicate scanner helpers instead of layering new helpers beside them

### Slice 5: Final Boundary Cleanup

- [ ] Re-scan the test tree for remaining repo-text assertions and remove them
- [ ] Do one `improve-code-boundaries` pass focused on deleting wrong-place support modules and single-caller helper fragmentation
- [ ] Make sure remaining tests speak only through real commands, images, runtimes, APIs, or typed in-memory results owned by the exercised boundary itself

### Slice 6: Validation

- [ ] Run `make check`
- [ ] Run `make lint`
- [ ] Run `make test`
- [ ] Run `make test-long` only if execution changed long-lane support or moved a required real behavioral check into that lane

## Expected Boundary Outcome

- The repo should lose a large amount of brittle support code that currently treats repo structure as a public API.
- The remaining test suite should be easier to refactor because behavior stays covered while file layout, helper names, docs wording, and artifact formatting stop being locked down.
- Support modules under `crates/runner/tests/support/` should become narrower and more honest owners of real boundaries instead of mixed runtime-plus-repo-scanner utilities.

Plan path: `.ralph/tasks/bugs/bug-remove-dumb-structure-and-file-content-tests_plans/2026-04-27-remove-dumb-structure-and-file-content-tests-plan.md`

NOW EXECUTE
