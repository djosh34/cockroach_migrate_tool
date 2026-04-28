## Task: Enable Development Without Host Nix <status>done</status> <passes>true</passes>

<description>
**Goal:** Add a Docker-based fallback for developers whose computers do not have Nix installed, where the container executes Nix inside the container and delegates to the same repository Nix flake used everywhere else. The higher order goal is to keep one canonical Nix workflow while still allowing contributors to build and test from machines that only have Docker.

In scope:
- add a Dockerfile or equivalent Docker build context specifically for running Nix inside a container
- make the fallback use the same Nix flake commands as native Nix development
- support common developer commands such as build, test, lint/check, and shell access through the containerized Nix environment
- document the fallback command path without reintroducing Make or non-Nix build logic
- keep this Dockerfile clearly separate from production image generation, which must be handled by Nix image outputs
- verify file ownership, cache behavior, and workspace mounts are usable enough for iterative local development

Out of scope:
- production Docker image generation
- CI publication workflows
- supporting legacy Make-based workflows

Decisions already made:
- this fallback exists only for computers without host Nix
- the fallback Dockerfile may exist even though production Dockerfiles are being removed, because its purpose is to run Nix in a container rather than build production images directly
- the fallback must not become a second independent build system

</description>


<acceptance_criteria>
- [x] A Dockerfile or equivalent Docker build context exists for running Nix inside a development container.
- [x] The fallback invokes the same Nix flake outputs used by native development for build, test, lint/check, and shell workflows.
- [x] The fallback is clearly named and documented as a no-host-Nix developer path, not a production image build path.
- [x] Manual verification: the containerized Nix environment can build the project from a clean checkout/workspace.
- [x] Manual verification: the containerized Nix environment can run the project tests and lint/check commands.
- [x] Manual verification: generated files, cache directories, and workspace ownership do not make the host checkout unusable after running the fallback.
- [x] No Make-based or non-Nix build logic is reintroduced by this fallback.
</acceptance_criteria>

<plan>.ralph/tasks/story-29-migrate-to-nix-crane/05-task-enable-development-without-host-nix_plans/2026-04-28-enable-development-without-host-nix-plan.md</plan>

<notes>
- Dev Dockerfile path: `docker/dev-nix/Dockerfile`
- Dev wrapper path: `scripts/dev-with-docker`
- Supported fallback commands: `build`, `check`, `lint`, `test`, `shell`
- Verification commands run:
  - `./scripts/dev-with-docker build`
  - `./scripts/dev-with-docker check`
  - `./scripts/dev-with-docker lint`
  - `./scripts/dev-with-docker test`
  - `./scripts/dev-with-docker shell`
  - `make check`
  - `make lint`
  - `make test`
- Docker fallback runtime note:
  - the container runs privileged so Nix can keep Linux sandboxing enabled inside Docker
- Ownership and cache observations:
  - no root-owned files remained in the checkout after the wrapper runs
  - persistent Docker volumes were populated and reused across runs
  - observed volume sizes after verification: store about 5.8G, db about 168M, home/cache about 14M
</notes>
