## Task: Migrate Docker Image Generation To Nix <status>not_started</status> <passes>false</passes>

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
- [ ] Nix produces each required project runtime image from the same crane-built artifacts used by local builds.
- [ ] Dockerfile-based production image generation is removed from the canonical workflow.
- [ ] Manual verification: each Nix image output builds successfully.
- [ ] Manual verification: each Nix-built image can be loaded into Docker or exported in the format needed by CI.
- [ ] Manual verification: each image starts successfully with its expected command/entrypoint and exposes the expected runtime behavior.
- [ ] Obsolete Dockerfiles, Dockerfile-only scripts, and documentation references are removed unless explicitly retained for the non-Nix developer fallback task.
- [ ] Task notes include the exact Nix commands used to build and inspect the images.
</acceptance_criteria>
