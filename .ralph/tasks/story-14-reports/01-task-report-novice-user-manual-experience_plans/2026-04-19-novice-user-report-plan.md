# Plan: Novice-User Manual Experience Report

## Scope

Produce the required report artifacts for `.ralph/tasks/story-14-reports/01-task-report-novice-user-manual-experience.md` by performing an actual README-first novice-user trial of the current system. This plan is for execution in a later turn; this turn does not perform the manual trial.

## Required Artifacts

- `.ralph/tasks/story-14-reports/artifacts/report-novice-user/summary.md`
- `.ralph/tasks/story-14-reports/artifacts/report-novice-user/step-by-step-experience.md`
- `.ralph/tasks/story-14-reports/artifacts/report-novice-user/friction-log.md`
- `.ralph/tasks/story-14-reports/artifacts/report-novice-user/recommendations.md`

## Public Interface Under Test

The public interface for this task is not internal Rust code. It is the novice operator experience exposed through:

- `README.md`
- documented `cargo run -p source-bootstrap -- render-bootstrap-script ...`
- documented `docker build` and `docker run ... validate-config`
- documented schema export, compare, helper-plan, postgres-setup, and runtime commands
- the documented cutover and verification contract when it becomes relevant to operator understanding

The report must treat the README path as authoritative first. Any extra investigation after README failure must be called out explicitly as friction, not silently normalized.

## TDD Execution Strategy

Use vertical slices. Do not read all code first and then write the report. For each slice:

1. `RED`: attempt the next README-guided operator behavior using only the documented public interface.
2. Observe the exact failure, hesitation, ambiguity, missing prerequisite, or success.
3. `GREEN`: do only the minimum extra investigation or setup needed to continue the manual trial.
4. Record the result immediately in the artifact drafts before moving to the next slice.

Each slice must test behavior through the operator surface, not implementation details.

## Behavior Slices To Execute

### Slice 1: README-only orientation

- Confirm what a novice can infer from the top-level README without reading code.
- Record whether the product purpose, workflow order, prerequisites, and source-vs-destination split feel obvious.
- Start `step-by-step-experience.md` and `friction-log.md` immediately with the first impression and any hesitation.

### Slice 2: Source bootstrap quick start

- Follow the source bootstrap example as written.
- Check whether a novice can understand required environment values, config shape, output expectations, and script review/execution flow.
- Record every command used, including any commands needed to create files or inspect outputs.
- If the example cannot be followed literally in the available environment, record the exact blocker and why.

### Slice 3: Docker quick start early path

- Follow the documented TLS generation, runner config creation, image build, and `validate-config` flow.
- Treat long or awkward commands as user friction even if they technically work.
- Record all pauses caused by unclear paths, mounts, env assumptions, binary availability, or config semantics.

### Slice 4: Schema and setup workflow

- Attempt the documented schema export, semantic compare, postgres setup render, and helper-plan render flow.
- Evaluate whether the sequence and prerequisites are obvious to a novice.
- Record whether README explains why each step exists before runtime startup.

### Slice 5: Runtime startup and mental model

- Attempt the documented runtime startup path as far as the environment allows.
- Evaluate whether the user can understand what `run` does automatically, what remains manual, and what successful startup should look like.
- Record any intimidation or ambiguity around helper tables, reconcile, verify, ingest paths, or cutover.

### Slice 6: Report synthesis

- Convert the raw notes into the four required artifacts.
- Ensure `summary.md` states plainly whether README alone was sufficient.
- Ensure `recommendations.md` prioritizes simplifications by actual observed friction, not implementer preference.

## Evidence Rules

- Every command used during execution must be captured in `step-by-step-experience.md`.
- Every hesitation, inference, missing prerequisite, or extra lookup must be captured in `friction-log.md`.
- Any step completed only after code-reading or cross-file investigation must state exactly where README stopped being enough.
- Do not soften or excuse friction.

## Improve-Code-Boundaries Rule

This task should avoid making the codebase muddier. Apply the `improve-code-boundaries` mindset during execution as follows:

- Prefer keeping operator-contract findings in the report artifacts unless a real implementation or documentation defect must be fixed to proceed.
- If a code or doc change becomes strictly necessary, remove duplication instead of adding parallel explanations in multiple places.
- Do not spread operator knowledge across ad hoc scratch files; keep the authoritative evidence in the required artifact files.
- Before closing the task, explicitly check whether any rescue change introduced duplicate command contracts, mixed responsibilities, or knowledge living in the wrong file.

## Validation And Finish

After the report artifacts are complete and any necessary fixes are done:

- run `make check`
- run `make lint`
- run `make test`
- run `make test-long`
- confirm the task file can be updated to `<passes>true</passes>` only after all four pass

## Expected Execution Notes

- The environment may not contain every external dependency needed for a literal end-to-end migration run. That is acceptable only if the report documents the exact missing prerequisite, why a novice would hit it, and whether README prepared the user for it.
- Because this is greenfield and backward compatibility is not a concern, any necessary follow-up fixes should prefer simplification over accommodation.

Plan path: `.ralph/tasks/story-14-reports/01-task-report-novice-user-manual-experience_plans/2026-04-19-novice-user-report-plan.md`

NOW EXECUTE
