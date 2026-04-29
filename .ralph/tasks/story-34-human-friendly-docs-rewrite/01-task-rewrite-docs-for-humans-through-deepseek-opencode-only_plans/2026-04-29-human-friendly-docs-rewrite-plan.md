# Plan: Rewrite The Human-Facing Docs With DeepSeek V4 Pro As Sole Author

## References

- Task:
  - `.ralph/tasks/story-34-human-friendly-docs-rewrite/01-task-rewrite-docs-for-humans-through-deepseek-opencode-only.md`
- Required skills read for this planning turn:
  - `.agents/skills/opencode/SKILL.md`
  - `.agents/skills/improve-code-boundaries/SKILL.md`
  - `.agents/skills/tdd/SKILL.md`
- Current human-facing docs and source-of-truth inputs sampled during planning:
  - `README.md`
  - `docs/public_image_operator_guide/`
  - `docs/setup_sql/index.md`
  - `docs/setup_sql/cockroachdb-source-setup.md`
  - `docs/setup_sql/postgresql-destination-grants.md`
  - `docs/tls-configuration.md`
  - `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
  - `scripts/ci/publish-quay-from-ghcr.sh`
  - `scripts/generate-cockroach-setup-sql.sh`
  - `openapi/verify-service.yaml`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`

## Planning Assumptions

- This turn is planning-only because the task file had no `<plan>` pointer and no execution gate.
- The task-level TDD exception applies because this is documentation work:
  - do not create brittle markdown-content tests
  - do not hand-author prose and then pretend to "test" it
  - still keep the TDD skill's discipline as an execution mindset:
    - verify one public-facing claim slice at a time
    - do not bulk-accept model output without proving it
    - stop immediately if the planned docs boundary proves wrong
- Never run `cargo`; use only the project Nix-backed commands if validation is needed.
- DeepSeek V4 Pro must be the only documentation prose author for this task.
- GLM 5.1 and Kimi 2.6 may only act as harsh critics during the final gate, never as authors.
- If any required reviewer model disappears before execution, fail instead of substituting another model.

## Available `opencode-go/*` Models Verified During Planning

- `opencode-go/deepseek-v4-pro`
- `opencode-go/glm-5.1`
- `opencode-go/kimi-k2.6`

Execution must record those exact identifiers again in the work log and use them for:

- author: `opencode-go/deepseek-v4-pro`
- critic: `opencode-go/glm-5.1`
- critic: `opencode-go/kimi-k2.6`

## Current State Summary

- The repo already has a canonical operator guide under `docs/public_image_operator_guide/`, but the task is not done because the docs are still not cleanly human-centered.
- The README currently owns too much low-level operator detail:
  - inline runner config
  - inline verify config
  - webhook payload contract
  - Docker Compose examples
  - endpoint details
- The current guide appears to have factual drift or at least contradictory image naming compared with the current README and CI scripts:
  - `README.md` uses `ghcr.io/${GITHUB_OWNER}/cockroach-migrate-runner:${IMAGE_TAG}` and `ghcr.io/${GITHUB_OWNER}/cockroach-migrate-verify:${IMAGE_TAG}`
  - `docs/public_image_operator_guide/image-references.md` still uses `ghcr.io/djosh34/runner-image:<git-sha>` and `ghcr.io/djosh34/verify-image:<git-sha>`
  - CI publication scripts still refer to logical image names `runner-image` and `verify-image`, plus Quay mirroring
- The docs boundary is therefore muddy:
  - README acts like both landing page and full reference
  - `docs/public_image_operator_guide/` acts like a separate full guide
  - `docs/setup_sql/` and `docs/tls-configuration.md` still hold operator-facing material that may need to be merged, replaced, or aggressively re-linked
- Because this is a greenfield project with zero users, execution should remove legacy or duplicate documentation structure rather than preserve it for compatibility.

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - human-facing documentation ownership is split across too many locations with overlapping operator guidance
- Boundary decision for execution:
  - README should become the short human landing page:
    - what the tool is
    - why it exists
    - where to go next
  - one canonical docs surface should own the deep operator material
  - DeepSeek V4 Pro may decide whether that canonical surface remains `docs/public_image_operator_guide/` or is renamed/restructured, but there must be only one obvious operator path when done
- Bold cleanup rule:
  - if `docs/setup_sql/` and `docs/tls-configuration.md` are fully subsumed by the new DeepSeek-authored structure, remove or replace them rather than keeping duplicate sources
  - if any docs page exists only because of earlier model-specific or transitional structure, delete it instead of preserving it

## Intended Reader Contract

- A human operator should be able to answer these questions quickly from the final docs:
  - what this project does
  - when to use it
  - what gets deployed
  - how to install or pull the right images
  - how to perform the supported setup path
  - how to configure runner
  - how to configure verify-service
  - how to understand the architecture at a useful level
  - where to find detailed config reference without hunting through the repo
- The README must route the reader to the right deeper docs instead of forcing them through low-level details inline.
- The deeper docs must be readable by a human who does not already know the repository layout.

## Execution Slices

### Slice 1: Factual Audit Before Authoring

- Inspect the authoritative sources needed to brief DeepSeek honestly:
  - image publication contract
  - runner CLI and runtime endpoints
  - verify-service CLI and HTTP API
  - setup SQL workflow
  - TLS/config surface
- Record exact evidence in a story-local artifact directory under:
  - `.ralph/tasks/story-34-human-friendly-docs-rewrite/artifacts/`
- Resolve the image-reference contradiction before any rewrite prompt.
- If image names or supported pull instructions still cannot be proven from authoritative sources, switch this plan back to `TO BE VERIFIED` and stop.

### Slice 2: Identify The Documentation Boundary To Replace

- Map the current human-facing docs surface:
  - what stays as source facts only
  - what becomes DeepSeek rewrite input
  - what is legacy/duplicate and a deletion candidate
- Make the boundary explicit in the prompt:
  - README is the landing page
  - one canonical deep-doc surface owns install/getting-started/config/architecture/reference
- If execution shows that more than one deep-doc surface is still necessary, switch back to `TO BE VERIFIED` instead of accepting a muddy split.

### Slice 3: DeepSeek V4 Pro Full Rewrite Pass

- Use `opencode run -m opencode-go/deepseek-v4-pro`.
- Tell DeepSeek explicitly:
  - the current docs are not human friendly
  - it is the sole allowed documentation prose author
  - it may aggressively reorder, merge, delete, and recreate docs
  - it must include:
    - intro
    - short problem/solution explanation
    - installation guidance
    - getting started guidance
    - configuration reference
    - architecture explanation
  - README should become a clear router, not a wall of low-level detail
- Ask DeepSeek for a file manifest with complete contents for every changed markdown file.
- Transcribe DeepSeek output verbatim without rewriting any prose.

### Slice 4: Human Verification Pass

- Only after DeepSeek has completed the rewrite may execution read the rewritten docs in detail.
- Assume every claim is wrong until proven.
- Verify:
  - image names and registries
  - tags and image pull workflow
  - commands and flags
  - ports and endpoints
  - config field names
  - TLS settings
  - SQL statements and placeholders
  - architecture statements about responsibilities and workflow
- Do not fix prose directly.
- Send every factual problem, missing section, bad ordering issue, or human-friendliness problem back to DeepSeek through `opencode` for another authored pass.

### Slice 5: DeepSeek Repair Loop

- If verification finds issues, run DeepSeek again with:
  - the current docs
  - the concrete findings
  - the instruction to rewrite the affected files itself
- Repeat Slice 4 and Slice 5 until the docs are both human-friendly and factually correct.

### Slice 6: Final Harsh Critic Gate

- Run all three critics through `opencode` on the final docs:
  - DeepSeek V4 Pro as harsh critic of its own final result
  - GLM 5.1 as independent harsh critic
  - Kimi 2.6 as independent harsh critic
- Ask each critic specifically whether:
  - the docs are good for humans
  - the README makes sense
  - installation is findable
  - getting started is findable
  - configuration reference is useful
  - architecture explanation is understandable
  - the ordering is sensible
- If any critic raises a substantial concern, return that feedback to DeepSeek V4 Pro for another rewrite, then repeat the entire critic gate.

### Slice 7: Validation And Cleanup

- Run:
  - `make check`
  - `make lint`
  - `make test`
- Do not run `make test-long` because this task is not a story-end validation gate and does not require the long lane.
- Run a final improve-code-boundaries review on the resulting docs structure:
  - if the docs still have duplicate ownership or stale transitional pages, clean that up before closing the task

## Stop Conditions

- Switch this plan back to `TO BE VERIFIED` immediately if:
  - the docs cannot be made truthful without product behavior changes
  - the image publication contract cannot be resolved honestly
  - the only way forward would require Codex to hand-author or directly rewrite documentation prose
  - DeepSeek's rewrite reveals that the chosen README-versus-guide boundary is still wrong
  - a required critic model is unavailable at execution time

## Final Verification Checklist For The Execution Turn

- [x] The exact model ids used are recorded in the task log:
  - `opencode-go/deepseek-v4-pro`
  - `opencode-go/glm-5.1`
  - `opencode-go/kimi-k2.6`
- [x] The executor inspected current docs and authoritative repository facts before briefing DeepSeek
- [x] DeepSeek V4 Pro fully rewrote and reorganized the docs
- [x] Codex did not directly write, rewrite, or polish any documentation prose
- [x] The final docs include intro, installation, getting started, config reference, and architecture explanation
- [x] The README acts as a clear landing page instead of a low-level wall of detail
- [x] Legacy, duplicate, or confusing docs were removed, merged, or recreated aggressively where needed
- [x] All factual claims were verified against authoritative repository sources
- [x] Any correction or clarity issue found by Codex was sent back to DeepSeek, not fixed by hand
- [x] DeepSeek V4 Pro agreed the final docs are good in the harsh critic pass
- [x] GLM 5.1 agreed the final docs are good in the harsh critic pass
- [x] Kimi 2.6 agreed the final docs are good in the harsh critic pass
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` not run
- [x] Only after all required checks pass may the task file be updated to `<passes>true</passes>`

Plan path: `.ralph/tasks/story-34-human-friendly-docs-rewrite/01-task-rewrite-docs-for-humans-through-deepseek-opencode-only_plans/2026-04-29-human-friendly-docs-rewrite-plan.md`

NOW EXECUTE
