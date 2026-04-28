## Task: Create fully new docs under a new `./docs/` directory using GLM 5.1 as the sole documentation author <status>not_started</status> <passes>false</passes>

<description>
Do not use TDD for this task because it is a documentation and manual verification task, not application code. Never run `cargo`; use Nix-backed validation commands only if local validation is needed.

**Goal:** Create a fully new documentation set for this project under one new directory inside `./docs/`, for example `./docs/public_operator_guide/`, with the exact directory name chosen during the task. The higher-order goal is to replace fragmented or insufficient operator guidance with a fresh, readable, independently usable documentation set that lets an operator set up the product directly from the published Docker image without relying on repository knowledge.

This task exists only to define the work. Do not start the docs work when creating this task. The future task executor must create the new docs directory first, then follow the staged model workflow below.

Non-negotiable authorship rule:
- The task executor must use the `opencode` tool for all model-authored writing and review passes.
- The task executor must only let GLM 5.1 write the docs text.
- The task executor, Codex, Claude, or any other local agent must not draft, rewrite, polish, or otherwise author the documentation prose.
- GLM 5.1 must have full artistic freedom over the documentation structure, wording, examples, diagrams, narrative flow, headings, and style, constrained only by factual correctness, the requirement to place everything under the new `./docs/{new-dir}/`, and the product's real behavior.
- If the exact model identifier is not known, the executor must run `opencode models | grep '^opencode-go/'` and choose the available `opencode-go/*` model that corresponds to GLM 5.1. If no GLM 5.1 model is available, the task must fail rather than silently using another writing model.

Required staged workflow:

1. Create the new documentation directory under `./docs/`.
   - The new directory must not reuse an existing docs directory.
   - All new docs for this task must live inside that one directory.

2. Ask GLM 5.1, through `opencode`, to write all docs in the new docs directory.
   - GLM 5.1 must be instructed that it is the sole documentation author and has full artistic freedom.
   - The executor may provide GLM 5.1 with factual source material from the repository, but must not supply prewritten documentation prose for GLM 5.1 to merely copy.
   - The output must cover the complete operator path needed to use the project from the published Docker image.

3. Ask GLM 5.1, through `opencode`, to improve the writing and flow of the docs it just wrote.
   - The pass must focus on making the docs easier to grasp, easier to read, and easier to visually parse.
   - GLM 5.1 should be explicitly invited to add or revise ASCII diagrams, flow diagrams, section order, examples, summaries, and cross-links where helpful.
   - The executor must still not personally rewrite or polish the docs text.

4. Only after GLM 5.1 has completed both writing passes may the executor read the generated docs.
   - The executor must assume the docs are wrong until proven correct.
   - The executor must perform deep verification against the actual repository behavior, configs, commands, Docker image contract, scripts, workflows, and any authoritative project source needed.
   - The executor must not alter the docs prose directly.
   - For every factual issue, ambiguity that could cause a failed setup, stale claim, missing prerequisite, unsafe command, wrong image reference, wrong config field, wrong SQL, wrong endpoint, wrong port, wrong environment variable, or unverifiable statement, the executor must add a local inline `<comment>...</comment>` block near the relevant text.
   - Each `<comment>` block must explain what appears wrong or unproven and cite the verified source or command that contradicts or fails to support it.

5. Ask DeepSeek V4 Flash, through `opencode`, to perform an independent setup attempt using only the docs.
   - DeepSeek V4 Flash may read only the new docs directory.
   - DeepSeek V4 Flash must not read the repository code, tests, scripts, workflows, existing docs, task files, or any other local project file outside the new docs directory.
   - DeepSeek V4 Flash must attempt to set the whole thing up directly from the published Docker image, exactly as an outside operator would.
   - If DeepSeek V4 Flash cannot complete setup from the docs alone, the executor must go back to GLM 5.1 through `opencode` and ask GLM 5.1 to improve the docs. Repeat the GLM 5.1 improvement and DeepSeek V4 Flash verification loop until the docs are sufficient or the task fails with a concrete blocker.
   - If the exact model identifier is not known, the executor must run `opencode models | grep '^opencode-go/'` and choose the available `opencode-go/*` model that corresponds to DeepSeek V4 Flash. If no DeepSeek V4 Flash model is available, the task must fail rather than silently using another independent setup model.

6. Finally, ask Kimi K2.6, through `opencode`, whether it likes the wording and whether it has stylistic language improvements.
   - Kimi K2.6 is only a style reviewer, not the primary author.
   - Any accepted style changes must be sent back to GLM 5.1 for final authorship, because only GLM 5.1 is allowed to write the docs text.
   - If the exact model identifier is not known, the executor must run `opencode models | grep '^opencode-go/'` and choose the available `opencode-go/*` model that corresponds to Kimi K2.6. If no Kimi K2.6 model is available, the task must fail rather than silently using another style-review model.

Out of scope:
- Implementing product code changes.
- Creating docs outside the one new `./docs/{new-dir}/` directory.
- Rewriting docs prose directly without GLM 5.1.
- Using Codex, Claude, DeepSeek, Kimi, or any other model as the docs author.
- Treating independent setup verification as optional.

Important project rules:
- Never ignore linter failures.
- Never skip required verification.
- Never swallow or ignore errors. Any discovered ignored/swallowed error anti-pattern in project code must be reported as an `add-bug` task.
- This is a greenfield project with zero users. Do not preserve legacy docs or legacy behavior for backwards compatibility if the task uncovers it as part of the docs work; create follow-up tasks or bugs as appropriate rather than documenting legacy as supported.
</description>

<acceptance_criteria>
- [ ] A new story-local docs directory exists under `./docs/{new-dir}/`, and all docs created by this task live only inside that directory.
- [ ] The task log or plan records the exact `opencode-go/*` model identifiers used for GLM 5.1, DeepSeek V4 Flash, and Kimi K2.6.
- [ ] GLM 5.1, invoked via `opencode`, authored the initial full documentation set with full artistic freedom.
- [ ] GLM 5.1, invoked via `opencode`, performed a second writing/flow improvement pass over the docs, including readability, graspability, visual parsing, and ASCII diagrams where useful.
- [ ] The executor did not write, rewrite, or polish the documentation prose directly.
- [ ] The executor read the docs only after GLM 5.1 completed both required writing passes.
- [ ] Deep verification was performed against authoritative project sources and commands, assuming the docs were wrong until proven correct.
- [ ] The executor did not alter docs prose during verification; every factual problem or unproven claim was marked only with an inline `<comment>...</comment>` block near the relevant text.
- [ ] Each `<comment>` block includes a concrete explanation and source/command basis for the concern.
- [ ] DeepSeek V4 Flash, invoked via `opencode`, attempted setup using only the new docs directory and no repository code or other project files.
- [ ] The DeepSeek V4 Flash setup attempt used the published Docker image directly rather than a local build.
- [ ] If DeepSeek V4 Flash could not complete setup from docs alone, the docs were returned to GLM 5.1 via `opencode` for improvement and then independently checked again.
- [ ] Kimi K2.6, invoked via `opencode`, reviewed the wording and suggested stylistic language improvements.
- [ ] Any accepted Kimi K2.6 style recommendations were applied only by sending them back to GLM 5.1 for final authored text.
- [ ] Manual verification confirms the final docs can stand alone for an outside operator using only the published Docker image.
- [ ] `make check` — passes cleanly, or the task fails with the full failing output recorded.
- [ ] `make lint` — passes cleanly, or the task fails with the full failing output recorded.
</acceptance_criteria>
