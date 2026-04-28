## Task: Migrate CI To Nix Only <status>completed</status> <passes>true</passes>

<description>
**Goal:** Replace the existing GitHub workflow with a Nix-only CI and publish pipeline that uses the same Nix/crane build graph as local development and produces one tagged multi-platform image. The higher order goal is to eliminate CI/local drift while keeping the publish path efficient, secure, and verifiable.

In scope:
- fully remove the existing GitHub workflow implementation and replace it with a Nix-only workflow
- reuse the repository Nix flake and crane outputs for build, test, lint, image generation, vulnerability verification, and publishing
- build both required architectures in parallel where GitHub Actions supports it
- produce one tagged multi-platform image from the per-architecture Nix-built images
- keep vulnerability verification with Trivy or the existing equivalent verification task
- keep publish tasks, but make them publish the Nix-built images rather than rebuilding images through Dockerfiles
- denote during task execution the GitHub variable and secret names needed to know where to publish
- ensure publish credentials are only available on the intended protected branch/event
- use authenticated workflow logs via `/home/joshazimullah.linux/github-api-curl` when real hosted verification is required
- ensure CI failures are explicit and never hidden behind ignored shell errors

Out of scope:
- changing registry strategy beyond what is needed to publish the Nix-built multi-platform image
- preserving old GitHub workflow jobs for backwards compatibility
- changing application code unless a separate task explicitly requires it

Decisions already made:
- CI must reuse the same Nix build as local development
- the workflow should create one tagged image that is multi-platform
- architecture builds should run in parallel for efficiency
- Trivy verification and publish stages must remain, but must operate on the Nix-built images
- the task implementer must record the exact GitHub variable/secret names needed for publishing while executing the task

</description>


<acceptance_criteria>
- [x] Existing GitHub workflow implementation is fully replaced by a Nix-only workflow.
- [x] CI build, lint/check, test, image generation, Trivy verification, and publish all use the repository Nix flake outputs.
- [x] Both target architectures are built in parallel jobs or an equivalent parallel matrix.
- [x] The workflow creates one tagged multi-platform image from the Nix-built per-architecture artifacts.
- [x] Publish stages reuse the already-built Nix image artifacts and do not rebuild via Dockerfiles.
- [x] Trivy or the existing vulnerability verification remains a required gate before publication.
- [x] The task notes or workflow comments denote the exact GitHub variables and secrets needed to choose the publish destination and authenticate publication.
- [x] Publish credentials are scoped to protected branch/event behavior and are not exposed to pull requests, forks, or unintended workflow triggers.
- [x] Manual verification: authenticated GitHub workflow logs show the Nix-only workflow running and either passing end-to-end or failing on a real unresolved issue captured as a follow-up bug/task.
</acceptance_criteria>

<plan>.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only_plans/2026-04-28-migrate-ci-to-nix-only-plan.md</plan>

<task_notes>
- Main implementation landed in commit `b96ea3a` (`wip: migrate ci to nix-only workflows`) and was then validated and repaired by follow-up commits on `master`, culminating in commit `9e53079ead9e0f7a634e8bc0d383de0a3d1a823f`.
- Canonical workflow inventory is now:
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/promote-image-tags.yml`
  - `.github/workflows/image-catalog.yml` removed
- Flake-owned workflow metadata and command surfaces used by GitHub Actions:
  - `nix eval --json .#github.publishImageCatalog`
  - `nix eval --json .#github.publishImageCatalog.publish_image_matrix`
  - `nix eval --json .#github.publishImageCatalog.release_image_catalog`
  - `nix run .#check`
  - `nix run .#lint`
  - `nix run .#test`
  - `nix build --no-link .#runner-image .#verify-image`
- Exact publish destination/authentication names documented in the workflow:
  - `vars.QUAY_ORGANIZATION`
  - `secrets.QUAY_ROBOT_USERNAME`
  - `secrets.QUAY_ROBOT_PASSWORD`
  - `secrets.GITHUB_TOKEN`
- Credential scoping kept on the protected publish path only:
  - publish jobs run only when `github.event_name == 'push' && github.ref == 'refs/heads/master'`
  - `workflow_dispatch` remains available for non-publish validation without Quay credentials
- Required local repo gates passed on the final tree:
  - `make check`
  - `make lint`
  - `make test`
- Hosted verification used the authenticated wrapper `/home/joshazimullah.linux/github-api-curl` against GitHub Actions run `#78`:
  - run id: `25028373105`
  - commit sha: `9e53079ead9e0f7a634e8bc0d383de0a3d1a823f`
  - started: `2026-04-28T01:11:57Z`
  - completed: `2026-04-28T01:26:16Z`
  - conclusion: `success`
- Hosted run `#78` completed all required Nix-only stages successfully:
  - `publish-catalog`
  - `validate-fast`
  - `validate-images`
  - four parallel `publish-image` matrix jobs across `linux/amd64` and `linux/arm64` for both runtime images
  - `quay-security-gate`
  - `publish-manifest`
</task_notes>
