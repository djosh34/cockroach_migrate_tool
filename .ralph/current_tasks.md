# Current Tasks Summary

Generated: Tue Apr 28 06:30:55 PM CEST 2026

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/bugs/bug-fix-nix-flake-check-without-docker.md`

```
## Bug: Fix nix flake check without Docker test assumptions <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
`nix flake check` is currently not fully green. The known failures include both verify-binary tests and some long-test related failures.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/01-task-create-magic-nix-cache-matrix-workflow-and-combine-image-artifacts.md`

```
## Task: Create Magic Nix Cache Matrix Workflow And Combine Image Artifacts <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Create a GitHub Actions workflow that runs five jobs in parallel, uses Magic Nix Cache everywhere Nix runs, builds per-architecture image artifacts for both `runner-image` and `verify-image`, runs `nix flake check` at the same time, then combines and publishes the per-architecture artifacts as exactly one multi-platform GHCR `runner-image` tag and one multi-platform GHCR `verify-image` tag after all five parallel jobs pass. The higher order goal is to make hosted CI fast, observable, and reproducible while publishing commit-SHA-tagged multi-platform images to GHCR without rebuilding images in the final assembly/publish step.
```

==============

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/02-task-publish-ghcr-built-multiplatform-images-to-quay-with-verbose-security-logs.md`

```
## Task: Publish GHCR Built Multiplatform Images To Quay With Verbose Security Logs <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Extend the image workflow after the GHCR publishing task so the same final multi-platform `runner-image` and `verify-image` images are also published to Quay. The higher order goal is to make the published images available from both GHCR and Quay while keeping the image build path single-source, avoiding rebuilds, and making Quay publish/security status visible directly in GitHub Actions logs.
```

