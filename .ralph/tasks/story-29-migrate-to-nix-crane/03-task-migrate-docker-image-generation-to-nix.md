## Task: Migrate Docker Image Generation To Nix <status>completed</status> <passes>true</passes>

<description>
**Goal:** Generate project Docker images with Nix instead of Dockerfiles, reusing the same crane/Nix build outputs that local builds and tests use. The higher order goal is to make container artifacts reproducible products of the Nix build graph rather than separate, drifting Dockerfile builds.

In scope:
- replace Dockerfile-based image generation with Nix image generation
- reuse the binaries and artifacts produced by the crane build
- produce the required project runtime images as Nix outputs
- remove obsolete Dockerfiles and Dockerfile-only build scripts unless one is explicitly kept for the separate "develop without Nix" fallback task
- preserve minimal runtime image behavior and avoid adding unnecessary shells, package managers, or tooling to production images
- ensure image metadata, entrypoints, exposed ports, users, and config expectations match the current product contract
- ensure Nix-built images can be loaded into Docker or exported for CI publication

Out of scope:
- GitHub Actions migration
- adding the fallback Dockerfile that runs Nix in a container for developers without host Nix
- preserving Dockerfile-based production image generation

Decisions already made:
- Dockerfiles must no longer be the production image build source
- Nix must provide the binaries that go into images
- no backwards compatibility is required for old image build commands

</description>


<acceptance_criteria>
- [x] Nix produces each required project runtime image from the same crane-built artifacts used by local builds.
- [x] Dockerfile-based production image generation is removed from the canonical workflow.
- [x] Manual verification: each Nix image output builds successfully.
- [x] Manual verification: each Nix-built image can be loaded into Docker or exported in the format needed by CI.
- [x] Manual verification: each image starts successfully with its expected command/entrypoint and exposes the expected runtime behavior.
- [x] Obsolete Dockerfiles, Dockerfile-only scripts, and documentation references are removed unless explicitly retained for the non-Nix developer fallback task.
- [x] Task notes include the exact Nix commands used to build and inspect the images.
</acceptance_criteria>

<plan>.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix_plans/2026-04-28-nix-image-generation-plan.md</plan>

<task_notes>
- Manual image build commands used:
  - `nix build .#runner-image --no-link --print-out-paths`
  - `nix build .#verify-image --no-link --print-out-paths`
- Manual Docker load commands used:
  - `docker image load -i "$(nix build .#runner-image --no-link --print-out-paths)"`
  - `docker image load -i "$(nix build .#verify-image --no-link --print-out-paths)"`
- Manual image inspect commands used:
  - `docker image inspect cockroach-migrate-runner:nix --format '{{json .Config.Entrypoint}}'`
  - `docker image inspect cockroach-migrate-verify:nix --format '{{json .Config.Entrypoint}}'`
- Manual runtime probes covered by focused ignored contracts:
  - runner: `validate-config --log-format json` and minimal filesystem export
  - verify: `verify-service validate-config --log-format json`, module-version inspection, and minimal filesystem export
- Required repo gates passed on the final tree:
  - `make check`
  - `make lint`
  - `make test`
- Story follow-up left intentionally for Task 04:
  - GitHub Actions publish workflow migration still needs to consume the flake image outputs instead of the removed Dockerfiles.
</task_notes>
