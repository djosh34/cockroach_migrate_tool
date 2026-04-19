## Task: Remove any novice-user path that requires a repo checkout, local installs, or build-from-source steps <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers removal of repo-download and local-tooling assumptions from the novice path
- [ ] Supported user-facing docs and tests no longer require cloning the repo or building images locally
- [ ] The remaining novice-user contract starts from published images pulled from the registry only
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
