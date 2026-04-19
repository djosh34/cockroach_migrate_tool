## Task: Verify a novice user can complete the supported flow from published images alone with zero repo access <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a stricter novice-user verification task that proves the supported operator path works when the user has Docker access and registry pull access only, after the published images and Compose artifacts already exist. The higher order goal is to make the published-image experience the real product, not an afterthought hidden behind repository knowledge.

In scope:
- verify a novice user can complete all supported actions from published images alone
- assume zero repository checkout and zero locally installed tooling beyond Docker/container runtime access
- verify the user does not need to read project coding rules, repository structure notes, or contributor guidance
- verify the supported path uses published images directly rather than local `docker build`
- verify the supported path may use separate published Docker Compose definitions for runner, verify, and SQL-emitter flows
- verify the novice user can work from the README examples without downloading the repository contents

Out of scope:
- contributor onboarding
- source-level development workflow

Decisions already made:
- the novice-user path must use the published images directly from the registry
- the user must not need anything besides pulling and running images
- README must stay operator-focused while contributor rules live elsewhere
- this story depends on the image-build and GitHub workflow publication stories being completed first
- Docker Compose is an allowed operator interface, including modern config-style features
- the README contains the novice-user examples and must be sufficient without a repo checkout

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers a registry-only novice-user journey from published images alone
- [ ] The task fails if the user must clone the repo, install extra tooling, or read contributor-only guidance
- [ ] The task fails if any supported novice-user step depends on a local image build instead of a published image
- [ ] The task verifies the novice user can use the separate published Compose contracts where applicable without a repo checkout
- [ ] The task verifies the novice user can follow the README examples directly without downloading the repository contents
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
