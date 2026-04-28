# Current Tasks Summary

Generated: Tue Apr 28 12:01:05 PM CEST 2026

# Task `/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/tasks/story-31-nix-github-workflow-cache-and-image-artifacts/01-task-create-ultra-simple-nix-github-workflow-with-cached-parallel-builds.md`

```
## Task: Create Ultra Simple Nix GitHub Workflow With Cached Parallel Builds <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Create an ultra simple GitHub Actions workflow that calls Nix directly for CI and image generation, uses Magic Nix Cache so dependencies and build outputs are reused across runs, and keeps image building and tests parallel. The higher order goal is to replace fragile custom CI image-build logic with a small, reproducible Nix-owned path where repeated runs do not rebuild everything and where the exact Nix-produced image artifact can later be uploaded to GHCR without rebuilding.
```

