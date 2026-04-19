## Task: Drive the full three-image GitHub pipeline under fifteen minutes with native `arm64` execution and aggressive workflow restructuring <status>not_started</status> <passes>false</passes>

<priority>ultra_high</priority>

<description>
Must use tdd skill to complete


**Goal:** The current hosted GitHub workflow run is still far too slow. A 20-plus-minute end-to-end run for the three-image pipeline is unacceptable. This task exists to redesign the GitHub Actions workflow structure so the full real pipeline for all images lands at fifteen minutes or less in hosted runs, while keeping the correctness gates intact. Fifteen minutes is only the bare minimum acceptable ceiling: the implementation should aim for roughly five minutes if that is achievable without weakening correctness or trust boundaries, and otherwise should drive the workflow to be as fast as practically possible. The higher order goal is to make the image pipeline fast enough to be operationally usable instead of merely less bad on paper.

In scope:
- move the `arm64` image build path off an `amd64` machine and onto native `arm64` execution in GitHub Actions or an equivalent repository-controlled native `arm64` runner path
- redesign the workflow graph so wall-clock runtime drops materially through better job decomposition, more parallelism, smarter fan-out and fan-in, and less duplicated waiting between lanes
- improve cache reuse across the full workflow, including validation, test, build, manifest, and publish-oriented jobs, rather than treating Dockerfile-local caching as the whole answer
- evaluate and implement speed wins beyond the Dockerfile layer, including matrix strategy changes, artifact handoff, workflow stage reordering, cache sharing, and any other GitHub workflow topology change that produces real runtime savings without weakening trust boundaries
- preserve real multi-image correctness for the runner, verify, and SQL-emitter image paths while making the hosted workflow faster
- add contract coverage that protects the intended fast workflow structure so later edits cannot silently collapse the runtime back into a serialized or cold-start-heavy path
- use real hosted GitHub workflow evidence to judge success instead of relying on local timing guesses

Out of scope:
- declaring victory from Docker-side cache changes alone if hosted wall-clock time is still too high
- marking the task complete based only on partial-job timing while the full pipeline remains too slow
- weakening test, validation, or publish correctness gates just to hit a runtime target
- pretending emulated `arm64` on `amd64` is acceptable for this task

Decisions already made:
- the current GitHub workflow runtime is still unacceptably slow
- a 20-plus-minute run is not acceptable
- the full hosted pipeline for all images must complete in fifteen minutes or less before this task may pass
- fifteen minutes is the bare minimum acceptable ceiling, not the true ambition
- the implementation should aim for roughly five minutes if practical, and otherwise push the workflow to be as fast as possible
- if the full real pipeline runtime is above fifteen minutes, this task must stay `<passes>false</passes>`
- the current `arm64` build path running on an `amd64` machine is a root cause that must be changed
- this task must aim for massive speedup in GitHub workflow structure, not just Dockerfile tuning
- solutions should consider more parallelism, broader cache reuse, and any other practical workflow-level optimization in addition to the native `arm64` split
- correctness and trust boundaries remain mandatory even while aggressively optimizing runtime
- this task belongs in story 21 and should take precedence over the rest of that story until it passes

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers the intended fast-path workflow structure and fails loudly if later edits re-serialize or de-cache the pipeline in ways that would predictably blow the runtime budget
- [ ] The hosted `arm64` image path no longer runs on an `amd64` machine; the workflow uses native `arm64` execution for the `arm64` build lane
- [ ] The implementation delivers real workflow-level speedups beyond Dockerfile-only tuning, such as improved parallelism, matrix topology, artifact handoff, stage reordering, or broader cache reuse
- [ ] Real hosted GitHub Actions evidence shows the full three-image pipeline, end to end for all required images, completes in fifteen minutes or less
- [ ] The optimization work explicitly aims well below the fifteen-minute ceiling, targeting roughly five minutes if feasible and otherwise the fastest practical hosted runtime
- [ ] This task is not marked `<passes>true</passes>` unless the full hosted pipeline runtime is fifteen minutes or less
- [ ] The faster workflow still preserves the required validation, test, build, manifest, and publish correctness gates
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure_plans/2026-04-20-native-arm64-under-fifteen-plan.md</plan>
