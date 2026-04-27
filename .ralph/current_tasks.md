# Current Tasks Summary

Generated: Tue Apr 28 12:33:58 AM CEST 2026

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`

```
## Task: Migrate Build Run Test And Lint To Crane <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Replace the current local build, run, test, check, and lint workflow with a fully reproducible Nix flake built around crane. The higher order goal is to make local development and CI share one Nix-native build graph where Rust artifacts are reused correctly and only the code that actually changed is rebuilt.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix.md`

```
## Task: Migrate Docker Image Generation To Nix <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Generate project Docker images with Nix instead of Dockerfiles, reusing the same crane/Nix build outputs that local builds and tests use. The higher order goal is to make container artifacts reproducible products of the Nix build graph rather than separate, drifting Dockerfile builds.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only.md`

```
## Task: Migrate CI To Nix Only <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Replace the existing GitHub workflow with a Nix-only CI and publish pipeline that uses the same Nix/crane build graph as local development and produces one tagged multi-platform image. The higher order goal is to eliminate CI/local drift while keeping the publish path efficient, secure, and verifiable.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-29-migrate-to-nix-crane/05-task-enable-development-without-host-nix.md`

```
## Task: Enable Development Without Host Nix <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Add a Docker-based fallback for developers whose computers do not have Nix installed, where the container executes Nix inside the container and delegates to the same repository Nix flake used everywhere else. The higher order goal is to keep one canonical Nix workflow while still allowing contributors to build and test from machines that only have Docker.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-30-optimize-rust-build-story/01-task-optimize-nix-crane-rust-dependency-builds.md`

```
## Task: Optimize Nix Crane Rust Dependency Builds <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete
```

