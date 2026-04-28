## Task: Create Ultra Simple Nix GitHub Workflow With Cached Parallel Builds <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Create an ultra simple GitHub Actions workflow that calls Nix directly for CI and image generation, uses Magic Nix Cache so dependencies and build outputs are reused across runs, and keeps image building and tests parallel. The higher order goal is to replace fragile custom CI image-build logic with a small, reproducible Nix-owned path where repeated runs do not rebuild everything and where the exact Nix-produced image artifact can later be uploaded to GHCR without rebuilding.

This is a workflow/infrastructure task, not an application-code task. TDD is not allowed for this task. The implementer must verify the workflow behavior manually and locally with Nix installed, and must not use test skipping or ignored errors as proof.

In scope:
- Add or replace the relevant GitHub workflow with an ultra simple workflow that installs/enables Nix and invokes repository Nix commands directly.
- Use Magic Nix Cache in the workflow to cache Nix dependencies and build outputs across GitHub Actions runs.
- Design the workflow so tests and image generation are separate jobs that can run in parallel.
- Ensure the workflow does not rebuild all dependencies and image layers from scratch on every run when previous Magic Nix Cache entries are available.
- Use the repository Nix image generation output as the source of truth for container images.
- After Nix image generation, retag the generated image to the correct image tag, which is always the exact git commit SHA.
- Ensure the final image reference is one single multi-platform image tag for that commit SHA.
- Ensure the multi-platform image is created via Nix first, before any registry upload logic is considered.
- Verify locally on this machine, which has Nix installed, that the Nix image generation and retagging flow actually works.
- Produce artifacts from the Nix image generation that a later GHCR workflow task can upload directly to GHCR without rebuilding.
- Document in task notes the exact Nix command(s), Docker/Skopeo/OCI command(s), artifact path(s), and local verification evidence used.

Out of scope:
- Publishing to GHCR.
- Adding GHCR credentials or registry write permissions.
- Designing release, promotion, or mutable tag policy.
- Reintroducing Dockerfile-based image builds.
- Preserving legacy workflow compatibility.

Decisions already made:
- The workflow must be intentionally small and must call Nix rather than duplicating build logic in shell.
- Magic Nix Cache is required.
- Cached Nix outputs from previous runs must be reused; repeatedly rebuilding everything is a task failure.
- Tests and image generation must be parallel jobs.
- Publishing to GHCR is a separate future task.
- The image tag must always be the git commit SHA.
- The final artifact must represent one multi-platform image tag, not separate user-facing per-architecture tags.
- The later GHCR upload task must consume the artifact produced by this Nix workflow directly and must not rebuild the images.

</description>


<acceptance_criteria>
- [ ] GitHub Actions contains an ultra simple Nix-based workflow for CI and image artifact generation.
- [ ] The workflow uses Magic Nix Cache and is structured so Nix dependencies/build outputs from previous runs are reused instead of rebuilding everything every run.
- [ ] Test execution and image generation are separate jobs that can run in parallel.
- [ ] Image generation uses repository Nix outputs directly; no Dockerfile-based rebuild path remains in this workflow.
- [ ] The Nix-generated image is retagged after generation to the exact git commit SHA.
- [ ] The image artifact represents one single multi-platform image tag for that commit SHA.
- [ ] The multi-platform image is created via Nix before any registry upload step would occur.
- [ ] The workflow stores the Nix-created multi-platform image artifact in a form that a later GHCR publish workflow can upload directly without rebuilding.
- [ ] Publishing to GHCR is not implemented in this task and is left as a separate task.
- [ ] Manual local verification: on this machine with Nix installed, run the Nix image generation and retagging flow successfully and record the exact commands and resulting artifact path in task notes.
- [ ] Manual cache/design verification: record how the workflow proves or observes Magic Nix Cache reuse across runs, using real hosted workflow logs where necessary.
- [ ] `make check` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
- [ ] `make lint` — passes cleanly unless the workflow-only nature of the change makes it inapplicable; if inapplicable, record the exact reason in task notes.
</acceptance_criteria>
