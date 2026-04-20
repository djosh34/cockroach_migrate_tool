## Task: Publish separate Docker Compose artifacts for the runner, verify, and SQL-emitter images using modern Compose config features <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Produce and publish separate Docker Compose definitions for each supported image path so operators can use published images directly without cloning the repository. The higher order goal is to make the image-first operator path concrete and ergonomic while keeping each runtime isolated in its own compose contract.

In scope:
- one Compose artifact for the runner image
- one Compose artifact for the verify image
- one Compose artifact for the one-time SQL-emitter image
- use of newer Docker Compose features, including config-style mounted configuration where appropriate
- workflow support so these compose artifacts are available alongside the published image flow
- README examples that show each Compose contract directly, so the novice user can copy the examples without a repo checkout

Out of scope:
- combining all three runtimes into one mandatory compose file
- local-development compose flows that depend on a repo checkout

Decisions already made:
- there must be a separate Docker Compose file for each thing
- the novice-user/operator path may use Docker Compose
- the compose definitions should use modern features such as config-style inputs
- the path must work from published artifacts and registry pulls rather than a repo clone
- the README already contains the examples/operators should be able to use that path rather than fetching extra files from the repo

</description>


<acceptance_criteria>
- [x] Red/green TDD covers separate Compose artifacts for runner, verify, and SQL-emitter usage from published images
- [x] Each Compose artifact is dedicated to one runtime and uses modern Compose config features where they fit the runtime contract
- [x] The supported operator path can consume these Compose artifacts without cloning the repository or building images locally, including via README examples
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
