## Task: Enable Development Without Host Nix <status>not_started</status> <passes>false</passes>

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
- [ ] A Dockerfile or equivalent Docker build context exists for running Nix inside a development container.
- [ ] The fallback invokes the same Nix flake outputs used by native development for build, test, lint/check, and shell workflows.
- [ ] The fallback is clearly named and documented as a no-host-Nix developer path, not a production image build path.
- [ ] Manual verification: the containerized Nix environment can build the project from a clean checkout/workspace.
- [ ] Manual verification: the containerized Nix environment can run the project tests and lint/check commands.
- [ ] Manual verification: generated files, cache directories, and workspace ownership do not make the host checkout unusable after running the fallback.
- [ ] No Make-based or non-Nix build logic is reintroduced by this fallback.
</acceptance_criteria>
