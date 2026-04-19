# Plan: Remove Contributor-Only Rules From README And Keep Them In Contributor Docs

## References

- Task: `.ralph/tasks/story-16-runtime-split-removals/05-task-remove-contributor-rules-from-readme-and-keep-them-in-contributors-docs.md`
- Current operator doc: `README.md`
- Current repo instructions: `AGENTS.md`
- Current README contract tests:
  - `crates/runner/tests/readme_contract.rs`
  - `crates/runner/tests/support/readme_contract.rs`
  - `crates/runner/tests/support/readme_published_image_contract.rs`
- Related prior plan:
  - `.ralph/tasks/story-16-runtime-split-removals/04-task-remove-novice-user-dependence-on-repo-clone-and-local-tooling_plans/2026-04-19-published-images-only-novice-path-plan.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the interface direction and test priorities in this turn.
- The README is an operator-facing public contract, not a contributor onboarding document.
- The current contributor-only content in `README.md` is the mixed internal/project-structure material:
  - `## Workspace Layout`
  - `## Command Contract`
- There is no existing dedicated contributor doc file, so this task should create a clear contributor-owned document boundary instead of leaving the material implicit or hidden in `AGENTS.md`.
- Preferred contributor-doc target: `CONTRIBUTING.md`.
- If execution shows that some current README material is still needed by operators, keep only the operator-facing subset in README and move the rest; do not preserve contributor guidance in README for backwards compatibility.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the README quick-start path stays operator-focused and no longer requires contributor workflow knowledge
  - contributor-only rules removed from README still exist in a contributor-owned doc
  - fast contract tests fail if internal repo-structure or contributor command guidance leaks back into the operator path
- Lower-priority concerns:
  - broader contributor-doc redesign beyond the moved material
  - expanding contributor guidance past what README currently exposes plus the minimal link from README to the contributor doc

## Problem To Fix

- `README.md` currently mixes two different audiences:
  - operator quick-start and runtime usage
  - contributor-facing project structure and local validation commands
- That mixed ownership weakens the novice-user contract established by earlier story-16 work because the public README still teaches repository internals and contributor gates.
- Current tests assert operator quick starts, but they do not explicitly fail when contributor-only guidance is reintroduced into README or when the moved guidance disappears entirely.

## Interface And Boundary Decisions

- Keep `README.md` operator-focused.
  - retain only the public/operator contract needed to pull images, render bootstrap SQL, validate config, render grants, and run the destination runtime
  - remove contributor-only sections that explain internal repository layout or local validation commands
- Introduce `CONTRIBUTING.md` as the contributor-owned doc boundary.
  - move the README’s contributor-only material there
  - allow contributor-specific language, repo structure, and local validation commands in that file
- README may include, at most, one short contributor redirect such as "For contributor workflow, see `CONTRIBUTING.md`."
- Add explicit doc-contract coverage for both sides of the boundary:
  - README forbids contributor-only markers
  - `CONTRIBUTING.md` preserves the moved rules

## Improve-Code-Boundaries Focus

- Primary smell: one public doc (`README.md`) currently owns two responsibilities that belong to different audiences.
  - operator contract
  - contributor workflow/rules
- Execution should flatten that mixed documentation boundary by moving contributor-only guidance into its own doc instead of teaching tests to tolerate an overloaded README.
- Secondary smell: README contract coverage protects operator quick starts, but there is no contributor-doc contract owner.
  - preferred fix: add a small docs-contract support boundary that can load repository docs and make doc-specific assertions without duplicating ad hoc string handling across tests

## Public Contract To Establish

- `README.md` does not require novice operators to understand:
  - `crates/runner`
  - `crates/source-bootstrap`
  - workspace/module layout
  - local validation commands like `make check`, `make lint`, `make test`, or `cargo ...`
- `README.md` remains sufficient for the operator path already validated by existing quick-start tests.
- `CONTRIBUTING.md` preserves the moved contributor guidance in a deliberate location.
- Contract tests fail loudly if:
  - contributor-only markers return to README
  - contributor docs stop documenting the moved guidance

## Files And Structure To Add Or Change

- [x] `README.md`
  - remove the contributor-only sections or replace them with a short contributor-doc pointer
- [x] `CONTRIBUTING.md`
  - add the moved project-structure and local validation guidance
- [x] `crates/runner/tests/readme_contract.rs`
  - strengthen README behavior coverage so contributor-only guidance in the operator doc fails fast
- [x] `crates/runner/tests/support/readme_contract.rs`
  - extend or reshape support helpers if execution needs a clearer repository-doc boundary
- [x] Add one contributor-doc contract test file if needed
  - preferred name: `crates/runner/tests/contributor_docs_contract.rs`
- [x] Add one support helper if needed
  - preferred owner: `crates/runner/tests/support/contributor_docs_contract.rs`

## TDD Execution Order

### Slice 1: Tracer Bullet For The README Boundary

- [x] RED: add one failing README contract test that rejects the current contributor-only markers in `README.md`
- [x] GREEN: remove the minimum contributor-only material from README so the operator doc becomes narrower and the new test passes
- [x] REFACTOR: keep README assertions scoped to operator-facing guarantees only

### Slice 2: Preserve The Removed Guidance In Contributor Docs

- [x] RED: add one failing contributor-doc contract test that requires the moved workspace-layout and local-validation guidance to exist in a contributor-owned document
- [x] GREEN: create `CONTRIBUTING.md` and move the contributor-only guidance there
- [x] REFACTOR: make the contributor-doc wording explicit about contributor intent rather than copying README prose blindly

### Slice 3: Protect The Operator Path From Internal-Structure Leakage

- [x] RED: add the next failing README assertion for repository-internal guidance that should never reappear in the novice path
- [x] GREEN: tighten README wording so it stays on the operator path only, optionally with a short `CONTRIBUTING.md` redirect
- [x] REFACTOR: extract any repeated doc-loading or marker assertions into a small docs-contract support owner if that reduces muddy test code

### Slice 4: Repository Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required default lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass on the doc/test ownership after the required lanes are green

## TDD Guardrails For Execution

- Every new assertion must fail before the supporting README or contributor-doc change is made.
- Do not satisfy this task by deleting contributor guidance outright; preserve it in `CONTRIBUTING.md`.
- Do not weaken the earlier novice-user contract by reintroducing repo-internal learning requirements into README.
- Do not add backwards-compatibility wording that keeps contributor rules in README "for convenience."
- Do not swallow missing-doc or missing-marker failures; they should fail loudly with clear messages.
- Do not run `make test-long` unless execution changes ignored long tests or their selection boundary; this task is expected to stay in the default lanes.

## Boundary Review Checklist

- [x] README owns only the operator-facing contract
- [x] Contributor-only workflow guidance lives in `CONTRIBUTING.md`
- [x] Tests protect both sides of the doc boundary
- [x] No repo-internal structure assumption remains in the novice-user README path
- [x] No moved contributor guidance is silently dropped

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` only if execution changes ignored long tests or their selection; this task is not expected to touch that boundary
- [x] One final `improve-code-boundaries` pass after the required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-16-runtime-split-removals/05-task-remove-contributor-rules-from-readme-and-keep-them-in-contributors-docs_plans/2026-04-19-contributor-doc-boundary-plan.md`

NOW EXECUTE
