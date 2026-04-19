## Task: Remove any novice-user path that requires a repo checkout, local installs, or build-from-source steps <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Remove every operator-facing path that assumes a novice user downloads the repository, installs local tooling, or builds images from source. The higher order goal is to make the supported novice-user flow work from published images alone with zero local project setup.

In scope:
- remove docs, tests, examples, and scripts that treat repo checkout as part of the novice-user path
- remove novice-user assumptions that Docker builds are done locally from repository source
- add verification that the supported operator flow starts from pulling published images only

Out of scope:
- contributor workflows and local developer setup

Decisions already made:
- a novice user must be able to do all supported actions from Docker images alone
- assume zero repo download for the novice-user path
- assume the only available action is pulling images from the registry

</description>


<acceptance_criteria>
- [x] Red/green TDD covers removal of repo-download and local-tooling assumptions from the novice path
- [x] Supported user-facing docs and tests no longer require cloning the repo or building images locally
- [x] The remaining novice-user contract starts from published images pulled from the registry only
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — not required; this task did not change the long-lane selection boundary
</acceptance_criteria>

<plan>.ralph/tasks/story-16-runtime-split-removals/04-task-remove-novice-user-dependence-on-repo-clone-and-local-tooling_plans/2026-04-19-published-images-only-novice-path-plan.md</plan>
