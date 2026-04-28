## Bug: Investigate transient Cachix 502 push failures in publish-images workflow <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
The `Publish Images` workflow now uses Cachix successfully instead of Determinate/Magic Nix Cache, but the authenticated hosted validation run still logged many transient Cachix push errors during the composite action post-step.

Evidence from hosted run `25077991160` on commit `4194d3738366caa09a0334d55eeeef103e1b06a8`:

- Workflow URL: `https://github.com/djosh34/cockroach_migrate_tool/actions/runs/25077991160`
- The Nix jobs configured Cachix correctly:
  - `Cache name: djosh34`
  - `Cache URI: https://djosh34.cachix.org`
- The workflow fully succeeded, including the final publish job.
- Despite that, the Cachix post-step logged repeated errors such as:
  - `statusCode = 502, statusMessage = "Bad Gateway"`
  - `Retry-After","60"`
  - `Failed to push /nix/store/...`

Representative log locations from the downloaded authenticated archive:

- `/tmp/publish-images-25077991160-logs/build runner-image (amd64)/9_Post Run ._.github_actions_setup-nix-cachix.txt`
- `/tmp/publish-images-25077991160-logs/build runner-image (arm64)/9_Post Run ._.github_actions_setup-nix-cachix.txt`
- `/tmp/publish-images-25077991160-logs/nix flake check/7_Post Run ._.github_actions_setup-nix-cachix.txt`

This must not be ignored just because the overall workflow stayed green. We need to understand whether the action retries are sufficient, whether some store paths never land in Cachix, and whether the workflow should fail or emit clearer reporting when Cachix push health degrades like this.
</description>

<mandatory_red_green_tdd>
TDD is not allowed for this bug because it is hosted GitHub Actions and external Cachix behavior, not application code.

Use manual hosted verification instead:

- inspect authenticated GitHub Actions logs with `/home/joshazimullah.linux/github-api-curl`
- reproduce with a real `push`-triggered workflow run
- verify whether Cachix push failures still appear and whether missing cache uploads can be confirmed from the hosted evidence
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I reproduced the transient Cachix push failure in an authenticated hosted workflow run or proved it no longer occurs
- [x] I determined whether failed `multipart-nar` uploads leave required store paths missing from cache
- [x] I documented whether the root issue is workflow configuration, Cachix action behavior, or a Cachix service-side availability problem
- [x] If a workflow-side fix is needed, I verified it with a real authenticated hosted run
- [x] I manually verified the final behavior from hosted logs rather than with brittle text-assert tests
- [x] `make check` — passes cleanly
- [x] `make lint` — passes cleanly
- [x] `make test` — passes cleanly
</acceptance_criteria>

<plan>.ralph/tasks/bugs/bug-investigate-transient-cachix-502-push-failures-in-publish-images-workflow_plans/2026-04-28-cachix-502-push-failure-investigation-plan.md</plan>

<execution_notes>
Hosted evidence:

- Prior failing run `25077991160` on commit `4194d3738366caa09a0334d55eeeef103e1b06a8` did contain real Cachix post-step failures in all three cache-pushing jobs:
  - `build runner-image (amd64)`
  - `build runner-image (arm64)`
  - `nix flake check`
- The failing requests were `POST /api/v1/cache/djosh34/multipart-nar` responses with Cloudflare-backed `502 Bad Gateway`, `retryable: true`, and `Retry-After: 60`.
- I downloaded the authenticated run logs and extracted 100 unique `Failed to push /nix/store/...` paths from that run.
- I then checked those exact paths against `https://djosh34.cachix.org`:
  - 99/100 are currently present in the project cache.
  - The one remaining miss is `/nix/store/sax580snmakhf2qb5s79k90bn1v9rsyn-cargo-src-whoami-1.6.1`.
- The missing path is a `cargo-src-*` source artifact, not a final published image or runtime package output.
- I separately verified that the important consumer-facing outputs from the failing run are present in `djosh34.cachix.org`, including:
  - both `runner-image` tarballs
  - both `verify-image` tarballs
  - both `runner-0.1.0` package outputs
  - both `verify-binary-0.1.4` package outputs

Fresh current-HEAD verification:

- Fresh push-triggered run `25078705823` on current `HEAD` `771a0b38b5566d1fccb84f0225f760b11954a92f` no longer reproduces the Cachix failure pattern in any cache-pushing job that has finished:
  - `build runner-image (arm64)`: `failed_push=0`, `status_502=0`
  - `build runner-image (amd64)`: `failed_push=0`, `status_502=0`
  - `nix flake check`: `failed_push=0`, `status_502=0`
- For both current `runner-image` jobs, the final docker layer and docker image tarball were pushed successfully in the Cachix post-step.

Conclusion:

- The repo workflow configuration is not the root cause.
- The local `.github/actions/setup-nix-cachix` boundary is already narrow and did not need changes.
- The prior failure is best explained as transient Cachix service-side availability degradation during multipart upload initiation, not a repo-owned workflow defect.
- No workflow-side fix was justified from the evidence, because the fresh authenticated hosted run on current `HEAD` is clean and adding local failure/reporting policy would add complexity for a transient upstream outage rather than fix a repo boundary bug.

Validation:

- `make check`
- `make lint`
- `make test`
</execution_notes>
