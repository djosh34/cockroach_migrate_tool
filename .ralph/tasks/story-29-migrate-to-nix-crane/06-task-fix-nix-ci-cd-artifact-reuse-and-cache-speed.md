## Task: Fix Nix CI/CD Artifact Reuse And Cache Speed <status>not_started</status> <passes>false</passes>

<description>
**Goal:** Fix the Nix/crane GitHub Actions CI/CD pipeline created under `story-29-migrate-to-nix-crane` so it is fast, cache-backed, and strictly artifact-reuse driven. The higher order goal is to make the hosted pipeline prove that Nix is the single build graph: each artifact is built once, reused by dependent jobs, tested through Nix, imaged through Nix, and published without any rebuild path.

This task exists because the previous CI/CD migration was not completed correctly. The current workflow takes far too long building Nix outputs and appears to rebuild work that should already be available from the Nix store, GitHub Actions cache, or explicit workflow artifacts.

This is a workflow/Nix task, not an application-code TDD task. Do not add fake source-level tests for workflow text. Verification must use real Nix commands locally where useful and authenticated GitHub Actions logs through `/home/joshazimullah.linux/github-api-curl` for hosted pipeline timing, rebuild, cache-hit, and publish evidence.

Required pipeline shape:
- First build the required Nix artifacts in parallel per platform.
- The test path must depend on the first build and then use Nix to build test outputs and run tests.
- The image path must also depend on the first build, run in parallel with the test path, and build all images in parallel.
- The publish path must depend only on already-built image artifacts and must only publish them.
- Any sense of rebuilding in publish is failure. Nix can and must reuse the already-built result.

Required caching behavior:
- Use the Nix-native way of fast caching and the appropriate GitHub Actions cache mechanisms for large speedups.
- Explicit PO override: remove ALL manual caching from the GitHub workflow. Do not use repo-maintained scripts, Python helpers, ad hoc cache save/restore steps, manual Nix store packing, or bespoke cache parsing for workflow caching. Use the well-liked Magic Nix Cache action instead, pinned to the latest verified tag at task alteration time: `DeterminateSystems/magic-nix-cache-action@v13`.
- A rerun of the pipeline with no relevant code changes must not rebuild external dependencies.
- A rerun after code changes must rebuild only the changed code-dependent artifacts and must never rebuild external dependencies that were already built in a previous run.
- The workflow must expose enough logs or checked evidence to prove which artifacts were built, which were copied/substituted/restored, and which derivation/output hashes were reused.

Hard failure conditions:
- No build may be done twice in one workflow run. Any redundant build of any artifact, checked by output hash or equivalent Nix store identity, is a full task failure.
- Publishing must not rebuild any artifact or image. Publish jobs may only download/copy/load/sign/push artifacts that were already produced by the image build jobs.
- Rebuilding any external dependency that was already built or cached in a previous run is a hard task failure.
- Ignored shell errors, best-effort cache restores that hide misses, or log parsing that silently skips missing data are bugs and must be reported with `add-bug` if discovered.

</description>


<acceptance_criteria>
- [ ] The GitHub workflow is restructured so the first stage builds all required platform-specific Nix artifacts in parallel.
- [ ] The Nix test/check stage depends on the first build stage, runs through Nix, and reuses first-stage artifacts instead of rebuilding them.
- [ ] The image stage depends on the first build stage, runs in parallel with the test/check stage, builds every required image in parallel, and reuses first-stage artifacts instead of rebuilding them.
- [ ] The publish stage depends on the image stage and only publishes already-built image artifacts; authenticated hosted logs prove publish performs no image or application rebuild.
- [ ] A workflow-level artifact/hash audit is added or otherwise made explicit so duplicate builds of the same artifact within one workflow run are detected and fail the run.
- [ ] Authenticated hosted GitHub Actions evidence proves there is no redundant build of any artifact in a single workflow run, using Nix output hashes, derivation/output paths, image digests, or an equivalent immutable identity.
- [ ] Total hosted build time for the build-heavy work is reduced to 5 minutes or less, measured from GitHub Actions job timing/log evidence.
- [ ] Total hosted wall-clock time end to end is 10 minutes or less, measured from GitHub Actions run start and completion timestamps.
- [ ] Nix caching is configured the Nix way and with direct GitHub Actions caching where appropriate, including large-cache behavior needed for fast reruns.
- [ ] The GitHub workflow contains no manual caching implementation: no repo-maintained caching scripts, no Python cache helpers, no manual Nix store archive/cache save/restore logic, and no bespoke cache parsing. It uses `DeterminateSystems/magic-nix-cache-action@v13` for GitHub Actions Nix caching.
- [ ] Authenticated hosted rerun evidence proves an unchanged rerun does not rebuild external dependencies and restores/substitutes them from cache/store instead.
- [ ] Authenticated hosted rerun evidence after a code-only change proves only code-dependent artifacts rebuild and no already-built external dependency rebuilds.
- [ ] The workflow fails loudly if required cache/hash/timing evidence cannot be collected; missing evidence must not be treated as success.
- [ ] Manual verification: use `/home/joshazimullah.linux/github-api-curl` to inspect the relevant GitHub Actions run logs and record the run ids, timestamps, job durations, cache evidence, artifact identities, image digests, and publish-no-rebuild proof in this task file or its plan notes.
</acceptance_criteria>
