## Bug: Investigate transient Cachix 502 push failures in publish-images workflow <status>not_started</status> <passes>false</passes> <priority>high</priority>

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
- [ ] I reproduced the transient Cachix push failure in an authenticated hosted workflow run or proved it no longer occurs
- [ ] I determined whether failed `multipart-nar` uploads leave required store paths missing from cache
- [ ] I documented whether the root issue is workflow configuration, Cachix action behavior, or a Cachix service-side availability problem
- [ ] If a workflow-side fix is needed, I verified it with a real authenticated hosted run
- [ ] I manually verified the final behavior from hosted logs rather than with brittle text-assert tests
- [ ] `make check` — passes cleanly
- [ ] `make lint` — passes cleanly
- [ ] `make test` — passes cleanly
</acceptance_criteria>
