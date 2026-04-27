## Task: Migrate CI To Nix Only <status>not_started</status> <passes>false</passes>

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
- [ ] Existing GitHub workflow implementation is fully replaced by a Nix-only workflow.
- [ ] CI build, lint/check, test, image generation, Trivy verification, and publish all use the repository Nix flake outputs.
- [ ] Both target architectures are built in parallel jobs or an equivalent parallel matrix.
- [ ] The workflow creates one tagged multi-platform image from the Nix-built per-architecture artifacts.
- [ ] Publish stages reuse the already-built Nix image artifacts and do not rebuild via Dockerfiles.
- [ ] Trivy or the existing vulnerability verification remains a required gate before publication.
- [ ] The task notes or workflow comments denote the exact GitHub variables and secrets needed to choose the publish destination and authenticate publication.
- [ ] Publish credentials are scoped to protected branch/event behavior and are not exposed to pull requests, forks, or unintended workflow triggers.
- [ ] Manual verification: authenticated GitHub workflow logs show the Nix-only workflow running and either passing end-to-end or failing on a real unresolved issue captured as a follow-up bug/task.
</acceptance_criteria>
