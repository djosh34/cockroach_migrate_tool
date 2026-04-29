## Task: Rewrite the docs for humans through DeepSeek opencode-only authorship <status>completed</status> <passes>true</passes>

<description>
Do not use TDD for this task because it is documentation work, not application code. Never run `cargo`; use Nix-backed validation commands only if local validation is needed.

**Goal:** Fix the current documentation because it is not human friendly, the README is confusing, and the docs do not make sense to a human operator trying to understand or use the project. The current docs overfocus on tiny low-level details instead of first explaining what the tool is, why it exists, how to install it, how to get started, how to configure it, and how the architecture works. The higher-order goal is to make the documentation make sense for humans: a reader should quickly understand the product, the supported operator workflow, and where to go for installation, getting started, configuration reference, and architecture details.

The task executor must treat the existing documentation as a source of facts only, not as a structure to preserve. The docs may be fully reordered, merged, deleted, recreated, and rewritten aggressively. Because this is a greenfield project with zero users and no backwards compatibility requirement, do not preserve legacy documentation structure, legacy wording, or legacy explanations for compatibility reasons. Remove or replace any legacy docs discovered during the work.

Non-negotiable authorship rule:
- The task executor itself MUST NOT edit, author, rewrite, polish, or directly change any documentation prose.
- ONLY DeepSeek V4 Pro, invoked through `opencode`, is allowed to write or rewrite documentation prose for this task.
- The executor may inspect the repository, gather facts, prepare factual source notes, run validation, and ask models to review the docs.
- The executor may not supply prewritten documentation prose for DeepSeek to copy. DeepSeek must be the documentation author.
- If the exact model identifier is not known, the executor must run `opencode models | grep '^opencode-go/'` and choose the available `opencode-go/*` model that corresponds to DeepSeek V4 Pro. If no DeepSeek V4 Pro model is available, the task must fail rather than silently using another writing model.

Required content and structure outcome:
- The docs must be tailored toward humans, not toward someone already familiar with the codebase.
- The docs must include a clear intro and short explanation of what this project is and what problem it solves.
- The docs must include getting started guidance.
- The docs must include installation guidance.
- The docs must include a configuration reference that is useful to an operator.
- The docs must include an architecture explanation that helps a reader understand the moving parts without drowning them in irrelevant implementation trivia.
- The docs must make clear what belongs in the README versus deeper docs, and the README should no longer feel confusing or overfocused on small details.
- DeepSeek should decide what would make sense to put in the docs, how to order it, what to merge, what to delete, and what to recreate, while staying factually correct about the project.

Required workflow:

1. Inspect the current documentation and repository facts enough to brief DeepSeek accurately.
   - This is research and verification only.
   - Do not directly edit docs during this step.
   - Identify the current README and docs files that confuse the human-facing story.

2. Ask DeepSeek V4 Pro, through `opencode`, to fully rewrite and reorganize the documentation.
   - DeepSeek must be told that the docs are currently not human friendly and must be made understandable for humans.
   - DeepSeek must be told to aggressively reorder, merge, delete, and recreate docs as needed.
   - DeepSeek must be told to include intro, short explanation, getting started, installation, config reference, and architecture explanation.
   - DeepSeek must be told that it is the only allowed documentation prose author for this task.

3. Verify the resulting docs against the repository and actual supported workflows.
   - The executor must assume the rewritten docs are wrong until proven correct.
   - The executor must verify commands, configuration fields, image references, endpoints, ports, environment variables, SQL snippets, scripts, and workflow claims against authoritative project sources.
   - The executor must not fix docs directly. Any factual corrections, missing sections, confusing ordering, or human-friendliness problems must be sent back to DeepSeek V4 Pro through `opencode` for DeepSeek to rewrite.

4. Repeat DeepSeek rewrite/review loops until the docs are coherent, human-friendly, and factually correct.
   - Do not accept merely cosmetic edits if the docs still do not make sense to a human.
   - The docs must be good enough that a harsh reviewer can find the expected high-level path and the expected reference material without already knowing the project.

5. Final model critic gate.
   - Ask DeepSeek V4 Pro, GLM 5.1, and Kimi 2.6 through `opencode` to independently review the final docs as very harsh and highly demanding critics.
   - Each reviewer must be asked whether the docs are good for humans, whether the README makes sense, whether getting started and installation are findable, whether the config reference is useful, whether the architecture explanation is understandable, and whether the docs are ordered sensibly.
   - The task is not complete until DeepSeek V4 Pro, GLM 5.1, and Kimi 2.6 all agree that the docs are good.
   - If any reviewer disagrees, raises a substantial concern, or says the docs are still confusing, send the feedback back to DeepSeek V4 Pro through `opencode` for another rewrite pass, then repeat the final model critic gate.
   - If any required reviewer model is unavailable, fail the task with the exact missing model and command output recorded. Do not silently substitute another model.

Out of scope:
- Product code changes.
- Direct documentation prose edits by the task executor, Codex, Claude, GLM, Kimi, or any model other than DeepSeek V4 Pro.
- Preserving old docs for backwards compatibility.
- Declaring the task complete when only one model likes the docs.
- Skipping final harsh critique from any of DeepSeek V4 Pro, GLM 5.1, or Kimi 2.6.

Important project rules:
- Never ignore linter failures.
- Never skip required verification.
- Never swallow or ignore errors. Any discovered ignored/swallowed error anti-pattern in project code must be reported as an `add-bug` task.
- This is a greenfield project with zero users. If legacy docs are found, remove or replace them as part of the DeepSeek-authored rewrite rather than preserving them.
</description>

<acceptance_criteria>
- [x] The task log or plan records the exact `opencode-go/*` model identifiers used for DeepSeek V4 Pro, GLM 5.1, and Kimi 2.6.
- [x] The executor inspected the current README/docs and repository facts before briefing DeepSeek V4 Pro.
- [x] DeepSeek V4 Pro, invoked through `opencode`, fully rewrote and reorganized the docs.
- [x] The executor did not directly write, rewrite, polish, or edit documentation prose.
- [x] The rewritten docs include a human-friendly intro and short explanation of what the project is.
- [x] The rewritten docs include findable getting started guidance.
- [x] The rewritten docs include findable installation guidance.
- [x] The rewritten docs include a useful operator-facing configuration reference.
- [x] The rewritten docs include an understandable architecture explanation.
- [x] The README no longer overfocuses on tiny implementation details and instead routes humans through the project clearly.
- [x] Legacy, confusing, duplicated, or badly ordered docs were merged, deleted, recreated, or reordered aggressively by DeepSeek V4 Pro as needed.
- [x] The executor verified factual claims against authoritative repository sources and supported workflows.
- [x] Any factual correction, missing section, bad ordering, or human-friendliness issue found by the executor was sent back to DeepSeek V4 Pro through `opencode`; the executor did not fix it directly.
- [x] DeepSeek V4 Pro reviewed the final docs as a very harsh, highly demanding critic and agreed the docs are good.
- [x] GLM 5.1 reviewed the final docs as a very harsh, highly demanding critic and agreed the docs are good.
- [x] Kimi 2.6 reviewed the final docs as a very harsh, highly demanding critic and agreed the docs are good.
- [x] If any final reviewer raised a substantial concern, DeepSeek V4 Pro rewrote the docs again and the full final critic gate was repeated.
- [x] `make check` — passes cleanly, or the task fails with the full failing output recorded.
- [x] `make lint` — passes cleanly, or the task fails with the full failing output recorded.
</acceptance_criteria>

<plan>.ralph/tasks/story-34-human-friendly-docs-rewrite/01-task-rewrite-docs-for-humans-through-deepseek-opencode-only_plans/2026-04-29-human-friendly-docs-rewrite-plan.md</plan>
