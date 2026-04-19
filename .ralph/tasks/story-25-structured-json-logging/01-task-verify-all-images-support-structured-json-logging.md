## Task: Verify every shipped image supports structured JSON logging and add any missing support needed for a consistent operator-facing logging contract <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Verify that every shipped image in the split-image product supports structured JSON logging suitable for machine parsing, and add any missing support so operators can rely on a consistent structured logging path across the full migration workflow. The higher order goal is to remove ad hoc human-only log formats from the supported operator surface and make log collection, filtering, and incident diagnosis dependable across runner, verify, setup, and any other shipped images.

In scope:
- identify every image that is part of the supported shipped product surface
- verify whether each image already supports structured JSON logging
- if an image does not support it yet, add the missing capability instead of documenting the gap as acceptable
- define the supported activation path for structured JSON logging across all images, whether by default behavior or an explicit config or flag
- verify that emitted logs are valid line-delimited JSON objects rather than mixed text plus JSON fragments
- verify that normal informational events and failure events both remain represented in structured JSON output
- verify that error cases are still logged explicitly and are not swallowed or downgraded into vague messages
- keep the structured logging contract simple enough that operators can configure log shippers without per-image special cases
- add or update automated coverage that locks the logging contract down

Out of scope:
- building a full external observability stack
- introducing high-cardinality or unstable log fields that make downstream processing brittle

Decisions already made:
- this must be a separate new story at the end of the backlog
- the goal is structured JSON logging for all shipped images, not only the runner
- a partial solution where some images stay plain-text-only is not acceptable for the supported operator path
- the logging contract should be consistent enough that operators do not need different parsing logic per image
- errors must remain visible and structured; no error may be swallowed to make JSON logging easier
- automated tests must prove JSON output validity and keep the supported logging surface stable
- any defect found during this verification that cannot be fixed inside the task must immediately create a bug via the `add-bug` skill
- when a bug is found, the verification flow must ask for a task switch so the system can switch to the bug task
- this task must not be marked passed unless every shipped image has a supported structured JSON logging path

</description>


<acceptance_criteria>
- [ ] Red/green TDD identifies every shipped image and verifies structured JSON logging support for each one
- [ ] Any shipped image that lacks structured JSON logging support is upgraded so the gap is removed rather than merely reported
- [ ] The task proves logs are valid line-delimited JSON objects for both normal operation and failure cases
- [ ] The task proves errors remain explicit, structured, and unswallowed in JSON logging mode
- [ ] The task defines and tests one supported operator-facing activation path for JSON logging across all images
- [ ] The task keeps the logging contract consistent enough that operators do not need per-image parser exceptions
- [ ] Every issue found during verification immediately results in a new bug task created via `add-bug`, and the workflow asks for a task switch to that bug
- [ ] `<passes>true</passes>` is allowed only if every shipped image supports the structured JSON logging path cleanly
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
