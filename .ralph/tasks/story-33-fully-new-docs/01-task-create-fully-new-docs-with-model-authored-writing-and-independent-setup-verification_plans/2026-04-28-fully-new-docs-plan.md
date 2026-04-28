# Plan: Create A New Canonical Public-Image Operator Guide With Model-Only Authorship

## References

- Task:
  - `.ralph/tasks/story-33-fully-new-docs/01-task-create-fully-new-docs-with-model-authored-writing-and-independent-setup-verification.md`
- Required skills read for this planning turn:
  - `.agents/skills/opencode/SKILL.md`
  - `.agents/skills/improve-code-boundaries/SKILL.md`
  - `.agents/skills/tdd/SKILL.md`
- Current operator-facing and runtime-contract sources:
  - `README.md`
  - `Makefile`
  - `openapi/verify-service.yaml`
  - `docs/tls-configuration.md`
  - `docs/setup_sql/index.md`
  - `docs/setup_sql/cockroachdb-source-setup.md`
  - `docs/setup_sql/postgresql-destination-grants.md`
  - `scripts/generate-cockroach-setup-sql.sh`
  - `scripts/README.md`
  - `.github/workflows/publish-images.yml`
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/ingest-contract/src/lib.rs`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Existing docs directories that are part of the fragmentation problem:
  - `docs/glm_5_1/`
  - `docs/deepseek_v4_pro_high/`
  - `docs/gpt_5_5_medium/`
  - `docs/kimi_k2_6/`

## Planning Assumptions

- This turn is planning-only because the task file had no `<plan>` pointer and no execution gate.
- The task-level TDD exception applies:
  - do not create brittle markdown string-comparison tests
  - do not treat documentation prose as something to be unit-tested
  - still keep the TDD discipline from the skill as an execution mindset:
    - one verification slice at a time
    - prove one public behavior at a time
    - do not bulk-change unrelated surfaces speculatively
- The execution turn must never run `cargo`; all local validation must stay on the Nix-backed project commands.
- The new canonical docs directory for this task will be `docs/public_image_operator_guide/` unless the pre-authoring factual audit shows that the name materially misrepresents the product surface.
- The exact available `opencode-go/*` models on this machine at planning time are:
  - author: `opencode-go/glm-5.1`
  - independent setup reviewer: `opencode-go/deepseek-v4-flash`
  - style reviewer: `opencode-go/kimi-k2.6`
- If any of those exact model ids disappear before execution, the task must fail instead of substituting another model.
- If the published-image registry contract cannot be resolved cleanly before the GLM authoring prompt, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- Operator guidance is fragmented across the README, separate TLS and setup-SQL docs, and multiple model-specific docs directories.
- Some existing model-authored docs are stale against current project rules:
  - `docs/glm_5_1/installation.md` still says `cargo build --release -p runner`
  - `docs/gpt_5_5_medium/installation.md` still mentions local Cargo builds
- The current supported runtime surface is image-first:
  - runner image
  - verify image
  - operator-managed CockroachDB setup SQL
  - operator-managed PostgreSQL grants
- The runner public CLI boundary currently includes:
  - `validate-config --config <PATH> [--deep]`
  - `run --config <PATH>`
  - optional `--log-format text|json`
- The runner HTTP boundary currently includes:
  - `GET /healthz`
  - `GET /metrics`
  - `POST /ingest/{mapping_id}`
- The verify-service public boundary currently includes:
  - `validate-config --config <path>`
  - `run --config <path>`
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /jobs/{job_id}/stop`
  - `GET /metrics`
- There is a real factual ambiguity around published image coordinates:
  - `README.md` still teaches `ghcr.io/${GITHUB_OWNER}/...`
  - recent workflow and task evidence show Quay publication work exists
- Because the docs must stand alone for an outside operator, execution must resolve that ambiguity before GLM writes anything registry-specific.

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - operator documentation ownership currently lives in the wrong places:
    - quick-start prose in `README.md`
    - setup SQL reference under `docs/setup_sql/`
    - TLS reference under `docs/tls-configuration.md`
    - stale model-specific docs directories with overlapping guidance
- Boundary decision for this task:
  - create one new canonical operator guide directory under `docs/public_image_operator_guide/`
  - let that directory own the full outside-operator flow from published image to verification
  - do not make `README.md` the owner of long-form operator guidance during this task
  - do not keep multiple competing model-authored operator guides once the new guide is verified
- Cleanup rule:
  - if execution confirms the older model-specific docs directories are legacy, stale, or contradictory, remove them rather than preserving parallel operator guides
  - do not remove `docs/tls-configuration.md` or `docs/setup_sql/` unless the new guide fully supersedes them and no remaining project surface depends on them

## Intended Public Contract

- The new docs must be independently usable by an outside operator who has:
  - access to the published Docker images
  - access to Docker
  - access to their CockroachDB and PostgreSQL environments
  - no repository knowledge beyond the new docs directory
- The new docs must cover, in whatever structure GLM chooses, the complete supported flow:
  - prerequisites and assumptions
  - how to identify the correct published image references
  - how to prepare source-side CockroachDB changefeed SQL
  - how to prepare destination-side PostgreSQL grants
  - how to write runner config
  - how to validate and run the runner image
  - how to write verify-service config
  - how to validate and run the verify image
  - how to start, poll, and stop verify jobs
  - relevant TLS guidance
  - health and metrics endpoints
  - common failure points and troubleshooting
- The docs must not depend on the README or any other external repo doc in order to complete setup.
- GLM 5.1 keeps full artistic freedom over structure and wording:
  - the execution turn may provide factual source material
  - the execution turn must not provide prewritten polished prose for GLM to copy

## Non-Authoring Constraints

- Codex must not draft, rewrite, or polish the docs prose.
- Codex may:
  - create the empty docs directory
  - prepare factual prompts
  - transcribe GLM-authored file contents into files verbatim
  - add inline verification-only `<comment>...</comment>` blocks
  - remove stale legacy docs directories if they are confirmed obsolete
  - run validation commands
- To keep authorship clean, execution should ask GLM to emit file manifests with full file contents, for example:
  - relative path under `docs/public_image_operator_guide/`
  - complete markdown body for each file
- When improvements are needed after DeepSeek or Kimi feedback, send those findings back to GLM and let GLM produce the revised text.

## Execution Slices

### Slice 1: Pre-Authoring Factual Audit

- Verify the actual source-of-truth inputs before any docs are written:
  - published image registry and repository naming
  - runner CLI and endpoints
  - verify-service CLI and endpoints
  - current setup SQL generation and recommended usage
  - TLS and config-file expectations
- Record the exact command and file evidence in a story-local artifact area such as:
  - `.ralph/tasks/story-33-fully-new-docs/artifacts/`
- If the registry/image contract is still contradictory after checking the authoritative sources, switch this plan back to `TO BE VERIFIED` and stop.

### Slice 2: Create The Empty Canonical Directory

- Create `docs/public_image_operator_guide/`.
- Do not read or edit any generated prose yet because none exists.
- Do not add README prose or other new docs outside that directory.

### Slice 3: GLM 5.1 Authoring Pass

- Use `opencode run -m opencode-go/glm-5.1`.
- Prompt GLM with:
  - the requirement that GLM is the sole documentation author
  - the exact target directory
  - the factual source pack from Slice 1
  - the requirement to cover the full outside-operator flow from published image to verification
  - freedom to choose file layout, diagrams, examples, and narrative flow
- Ask GLM to output a file manifest with complete file contents for every doc it wants under `docs/public_image_operator_guide/`.
- Transcribe that output into files without rewriting the prose.

### Slice 4: GLM 5.1 Flow-Improvement Pass

- Use `opencode run -m opencode-go/glm-5.1` again.
- Provide GLM only:
  - the docs it just authored
  - the instruction to improve readability, flow, summaries, diagrams, cross-links, and visual scanability
- Ask GLM for revised full file contents or a clean file manifest replacement.
- Transcribe the revised output without personal rewriting.
- Only after this pass is complete may Codex read the generated docs in detail.

### Slice 5: Deep Human Verification With Inline Comments Only

- Read the generated docs assuming they are wrong until proven right.
- Verify every concrete claim against the real product sources and commands.
- For each wrong, missing, unsafe, stale, or unproven claim:
  - insert a local inline `<comment>...</comment>` block near the relevant text
  - cite the authoritative repo file, workflow, script, test, or command result that supports the concern
- Do not rewrite surrounding prose directly.
- Typical high-risk areas to verify:
  - image references and tag expectations
  - source-side SQL examples
  - destination grant examples
  - CLI commands and flags
  - ports and endpoints
  - TLS field names
  - verify job lifecycle wording
  - health and metrics endpoints

### Slice 6: DeepSeek V4 Flash Independent Setup Attempt

- Use `opencode run -m opencode-go/deepseek-v4-flash`.
- Provide only the contents of `docs/public_image_operator_guide/`.
- Do not expose repo files, scripts, tests, workflows, or README text outside the new docs directory.
- Instruct DeepSeek to act like an outside operator using only the published Docker image contract described by the docs.
- Have it report:
  - where setup succeeds
  - where it becomes blocked
  - what assumptions are undocumented or ambiguous
- If DeepSeek cannot complete the supported flow from the docs alone, feed its findings back to GLM 5.1 and repeat Slice 4, then Slice 5, then Slice 6.

### Slice 7: Kimi K2.6 Style Review

- Use `opencode run -m opencode-go/kimi-k2.6`.
- Provide the new docs directory contents only.
- Ask Kimi only for language/style improvements and clarity notes, not factual rewrites.
- Triage the suggestions:
  - reject anything that changes facts or broadens scope
  - send accepted style feedback back to GLM 5.1 for final authored text
- Transcribe only the resulting GLM-authored revisions.

### Slice 8: Boundary Cleanup Pass

- Confirm the new docs directory is the clear canonical owner of full operator guidance.
- Remove legacy model-specific docs directories if they are now redundant or misleading:
  - `docs/glm_5_1/`
  - `docs/deepseek_v4_pro_high/`
  - `docs/gpt_5_5_medium/`
  - `docs/kimi_k2_6/`
- Do not invent a second guide or keep parallel competing model-authored operator walkthroughs.
- If cleanup would require new explanatory prose outside the new directory, switch back to `TO BE VERIFIED` instead of violating the authorship boundary.

### Slice 9: Default Validation Lanes

- Run only the required default project lanes for task completion:
  - `make check`
  - `make lint`
  - `make test`
- Do not run `make test-long` because this is not a story-end task and the task does not require the long lane.
- If any required lane fails, record the real failure and do not mark the task complete.

## Stop Conditions

- Switch this plan back to `TO BE VERIFIED` immediately if:
  - the published image contract cannot be resolved honestly
  - the guide needs product behavior changes to become truthful
  - the only apparent way forward is for Codex to hand-author or rewrite docs prose
  - the DeepSeek docs-only setup loop reveals that the chosen single-directory boundary is materially wrong
  - cleanup of stale docs requires a wider repo-wide documentation rewrite outside the permitted authorship boundary

## Final Verification Checklist For The Execution Turn

- [ ] `docs/public_image_operator_guide/` exists and all newly created docs for this task live only inside it
- [ ] The exact model ids used are recorded:
  - `opencode-go/glm-5.1`
  - `opencode-go/deepseek-v4-flash`
  - `opencode-go/kimi-k2.6`
- [ ] GLM 5.1 authored the initial doc set
- [ ] GLM 5.1 authored the readability/flow improvement pass
- [ ] Codex did not personally write or polish the docs prose
- [ ] Codex only read the docs after both GLM passes completed
- [ ] Every factual problem or unproven claim is marked only with inline `<comment>...</comment>` blocks
- [ ] DeepSeek V4 Flash attempted setup using only the new docs directory
- [ ] DeepSeek used the published Docker image contract rather than a local source build
- [ ] Accepted Kimi style feedback was applied only through a final GLM-authored revision
- [ ] Legacy model-specific docs directories were removed if they were confirmed redundant or stale
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` not run
- [ ] Only after all required lanes pass may the task file be updated to `<passes>true</passes>`

Plan path: `.ralph/tasks/story-33-fully-new-docs/01-task-create-fully-new-docs-with-model-authored-writing-and-independent-setup-verification_plans/2026-04-28-fully-new-docs-plan.md`

NOW EXECUTE
