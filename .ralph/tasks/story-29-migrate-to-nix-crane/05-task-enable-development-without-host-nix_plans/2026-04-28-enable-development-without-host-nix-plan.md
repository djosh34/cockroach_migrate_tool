# Plan: Enable Development Without Host Nix

## References

- Task:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/05-task-enable-development-without-host-nix.md`
- Prior story steps:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane_plans/2026-04-28-migrate-build-run-test-lint-to-crane-plan.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only.md`
- Current public workflow surfaces:
  - `flake.nix`
  - `Makefile`
  - `README.md`
  - `.dockerignore`
- Existing Docker/runtime examples that must not be confused with this task:
  - `artifacts/compose/runner.compose.yml`
  - `artifacts/compose/verify.compose.yml`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn started with no task-05 plan artifact and no `<plan>` path in the task file, so this is a planning turn and must stop after the plan is written.
- The canonical developer build graph already exists in `flake.nix`.
  - `check`, `lint`, `test`, `test-long`, `runner`, `verify-service`, `runner-image`, `verify-image`, and `devShells.default` are already flake-owned.
- The no-host-Nix fallback must execute that exact flake graph inside Docker.
  - It must not reintroduce Cargo, Go, or Make as independent orchestration paths.
- This task is code and workflow work, so the `tdd` skill applies.
  - Verification must use executable Docker and Nix commands through the public surface.
  - Do not add fake tests that assert strings in Dockerfiles, scripts, or docs.
- `make check`, `make lint`, and `make test` remain the mandatory final repo gates on the execution turn.
  - They prove the final repository state is healthy.
  - They are not the way the no-host-Nix fallback itself will be validated.
- `make test-long` remains out of scope for normal completion of this task.
- No backwards compatibility is allowed.
  - Do not keep an old host-tooling or Dockerfile-based build path alive "just in case."
  - If an existing doc path implies Docker builds production images directly, rewrite or remove that implication.
- If the first execution slice proves the fallback cannot honestly keep host file ownership and container cache state usable without a different interface than planned here, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Verification Priorities

- Highest-priority public behaviors to prove:
  - a machine with Docker but without host Nix can build a dedicated development container
  - that container can execute the same flake-backed build path as native Nix
  - that container can execute the same flake-backed `check`, `lint`, and `test` paths
  - that container can open a real `nix develop` shell against the mounted workspace
  - the fallback leaves generated host files owned by the calling host user rather than root
  - the fallback keeps Nix/cache/workspace mounts usable for iterative local development
  - the fallback is documented as developer-only and clearly separated from Nix-built production image outputs
- Lower-priority behaviors:
  - extra convenience features beyond the explicit `build`, `check`, `lint`, `test`, and `shell` surfaces
  - elaborate container orchestration if one reusable boundary is enough

## Current State Summary

- `flake.nix` already exposes the canonical Nix-native commands and a `devShells.default`.
- `Makefile` is intentionally a thin Nix-backed shim:
  - `make check` -> `nix run .#check`
  - `make lint` -> `nix run .#lint`
  - `make test` -> `nix run .#test`
- The repository no longer uses Dockerfiles for production runtime images.
  - Production images come from `flake.nix` via `pkgs.dockerTools.buildImage`.
- `README.md` currently teaches published runtime image usage, not contributor development without host Nix.
- There is no current Docker-based dev fallback boundary.
  - A contributor without host Nix has no honest way to run the flake locally today.

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - the repository has one canonical Nix workflow but no single place that owns Docker-specific developer bootstrap concerns for users without host Nix
  - if we solve this only with repeated README command blocks, the repo will spread user mapping, cache mounts, workspace mounts, and flake command selection across docs
- Desired boundary after execution:
  - `flake.nix` remains the single owner of build/test/lint/dev-shell behavior
  - one small Docker fallback boundary owns only:
    - base image choice
    - enabling `nix-command` and `flakes`
    - workspace mount location
    - host uid/gid propagation strategy
    - cache/store mount strategy
  - docs describe that boundary rather than duplicating long `docker run` command soups everywhere
- Smells to actively avoid:
  - smell 3, wrong place-ism:
    - do not move flake command knowledge into Docker-only scripts or docs
  - smell 10, remove the damn helpers:
    - only add a wrapper script if it is the real reusable public boundary for multiple developer commands
    - do not add a pile of one-off helper scripts with one caller each
  - smell 6, mixed responsibilities:
    - keep "build the dev container" separate from "run a selected flake command inside it"
- Working conclusion for execution:
  - prefer one dedicated Docker build context plus one reusable command surface for running flake commands inside it
  - avoid Docker Compose unless execution proves plain `docker build` plus `docker run` cannot keep the interface coherent

## Proposed Public Interface

- Docker build context:
  - add a clearly named dev-only path such as `docker/dev-nix/Dockerfile`
  - the name must make it obvious this is for developer tooling, not runtime image publishing
- Reusable dev entry surface:
  - prefer one script such as `scripts/dev-with-docker`
  - supported subcommands:
    - `build`
    - `check`
    - `lint`
    - `test`
    - `shell`
  - script responsibilities:
    - build the dev image if needed or document the exact image tag to build first
    - run the container with the mounted repo as working tree
    - pass through the calling user uid/gid so host-owned files stay usable
    - mount persistent Docker volumes for the Nix store/cache state where that improves iteration speed
    - dispatch to the already-existing flake commands only
- Commands owned by the fallback boundary:
  - `build` -> `nix build --no-link .#runner .#verify-service`
  - `check` -> `nix run .#check`
  - `lint` -> `nix run .#lint`
  - `test` -> `nix run .#test`
  - `shell` -> `nix develop`
- Commands explicitly not owned by the fallback boundary:
  - production image generation
  - publish workflows
  - any non-Nix build path

## Type And Interface Decisions

- Prefer a single explicit script entrypoint rather than duplicating five long `docker run` examples in docs.
  - This is acceptable because the script does not own build logic; it owns Docker bootstrap and then calls the flake.
- Prefer a fixed workspace path inside the container, for example `/workspace`.
  - That keeps the mounted flake path stable across subcommands.
- Prefer explicit environment setup inside the container rather than hidden host preconditions:
  - enable `nix-command` and `flakes`
  - set a writable `HOME`
  - keep any cache/state path obvious in the script or Dockerfile
- Prefer named Docker volumes for expensive reusable state when possible:
  - Nix store/cache volume
  - optional cargo/build cache volume only if execution proves it is needed and still truthful to the flake-driven path
- Prefer host uid/gid pass-through on `docker run`.
  - Do not rely on root in the mounted repo.
  - Do not leave root-owned files behind in the checkout.
- Keep the fallback separate from `Makefile`.
  - `Makefile` should stay a host-Nix shim for the repo gates.
  - Do not make `make` silently pick Docker on some machines and Nix on others.

## TDD Execution Strategy

- This task is a workflow/bootstrap task, so TDD must use real command execution rather than repo-string assertions.
- Tracer-bullet philosophy:
  - prove one mounted-workspace Docker container can execute one real flake command
  - only then expand to the other developer commands
- For each slice:
  - RED:
    - run the real Docker or Docker-plus-Nix command and capture the failure
  - GREEN:
    - add the minimal Dockerfile/script/doc change needed to make only that behavior pass
  - REFACTOR:
    - collapse repeated Docker invocation details into the single chosen fallback boundary
- Do not add Rust or shell tests that only inspect file contents.
  - Manual verification for this task is the truthful public contract.

## Vertical Execution Slices

### Slice 1: Dev Container Tracer Bullet

- [ ] RED:
  - run `docker build` against the planned dev Dockerfile path and confirm it fails because the build context does not exist yet
- [ ] GREEN:
  - add the dedicated dev Dockerfile
  - ensure the built image can start with Nix and see the mounted repository
- [ ] Verification:
  - build the image
  - run one real flake-backed command from the mounted repo, preferably `nix build --no-link .#runner`
- Stop condition:
  - if the image choice or container bootstrap cannot execute the repository flake honestly, switch back to `TO BE VERIFIED`

### Slice 2: Reusable Docker Command Boundary

- [ ] RED:
  - demonstrate that repeating raw `docker run` invocations for each command is already awkward enough to justify one reusable boundary
- [ ] GREEN:
  - add one reusable entry surface, preferably `scripts/dev-with-docker`
  - keep it responsible only for Docker concerns and flake-command dispatch
- [ ] REFACTOR:
  - inline or delete any tiny one-off helper fragments so the interface stays single-surface
- [ ] Verification:
  - run the new fallback interface for at least `check` and `shell`

### Slice 3: Flake Command Coverage Inside Docker

- [ ] RED:
  - run the planned fallback commands and let the first missing or miswired flake invocation fail honestly
- [ ] GREEN:
  - wire the reusable boundary so it runs:
    - `build`
    - `check`
    - `lint`
    - `test`
    - `shell`
- [ ] REFACTOR:
  - keep the command dispatch table small and explicit
  - do not add alternate Cargo, Go, or Make branches
- [ ] Verification:
  - exercise all supported public subcommands at least once, using real Docker execution

### Slice 4: Ownership And Cache Behavior

- [ ] RED:
  - run the fallback against the mounted repo and observe whether it leaves root-owned files or unusable cache state
- [ ] GREEN:
  - fix uid/gid propagation and writable home/cache/store mounts
  - keep the host checkout usable after the container exits
- [ ] REFACTOR:
  - remove any ad hoc chmod/chown cleanup steps if the runtime interface can prevent the problem directly
- [ ] Verification:
  - inspect created files and directories after `check` or `test`
  - confirm the fallback can be run twice without degenerating into a cold rebuild every time unless the flake truly requires it

### Slice 5: Documentation And Naming Boundary

- [ ] RED:
  - identify the current docs gap for contributors without host Nix
- [ ] GREEN:
  - document the fallback in `README.md`
  - make the text explicit that:
    - this is for contributor development on machines without host Nix
    - production images are still built by Nix image outputs, not by this Dockerfile
    - the fallback delegates to the same flake commands as native Nix
- [ ] REFACTOR:
  - remove or rewrite any wording that makes this dev Dockerfile look like a production image path
- [ ] Verification:
  - follow the documented steps from a clean checkout path as literally as possible

### Slice 6: Final Validation And Boundary Review

- [ ] Run the required repo gates:
  - `make check`
  - `make lint`
  - `make test`
- [ ] Run the task-specific manual Docker fallback verification:
  - clean-ish checkout containerized build
  - containerized `check` / `lint` / `test`
  - containerized `shell`
  - ownership and cache review
- [ ] Update task notes with:
  - exact dev Dockerfile path
  - exact fallback entry surface and commands
  - exact Docker validation commands executed
  - observed ownership/cache behavior
- [ ] Final `improve-code-boundaries` review:
  - confirm the flake still owns the build graph
  - confirm Docker bootstrap owns only Docker bootstrap
  - confirm no second independent build system was introduced

## Expected File Shape After Execution

- New:
  - `docker/dev-nix/Dockerfile` or an equivalently explicit dev-only Docker context
  - `scripts/dev-with-docker` if the reusable boundary is justified during execution
- Existing files likely to change:
  - `README.md`
  - `.dockerignore`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/05-task-enable-development-without-host-nix.md`
- Existing files that should not gain fallback-specific build logic:
  - `flake.nix`
    - may gain small ergonomic outputs only if truly necessary
    - must remain the single owner of build/test/lint logic
  - `Makefile`
    - should not become a Docker-or-Nix switchboard

## Expected Outcome

- Contributors with Docker but without host Nix get one honest development path that still runs the canonical flake.
- The repository keeps one build system.
- Docker-specific concerns are isolated behind one small boundary instead of being spread across docs and ad hoc commands.
- The fallback is clearly marked as developer-only and does not regress the Nix-owned production image story.

Plan path: `.ralph/tasks/story-29-migrate-to-nix-crane/05-task-enable-development-without-host-nix_plans/2026-04-28-enable-development-without-host-nix-plan.md`

NOW EXECUTE
