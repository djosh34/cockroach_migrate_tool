## Task: Replace Magic Nix Cache and FlakeHub Touchpoints with Cachix <status>not_started</status> <passes>false</passes>

<description>
Do not use TDD for this task because it is GitHub Actions/Nix workflow configuration, not application code. Do not run `cargo`; use Nix-backed commands only when local validation is needed.

**Goal:** Replace the current Determinate Magic Nix Cache setup in `.github/workflows/publish-images.yml` with Cachix so the multiplatform image workflow stops depending on Magic Nix Cache, avoids FlakeHub login attempts, and uses the project's Cachix binary cache instead. The Cachix cache name from the setup instructions is `djosh34`. The GitHub Actions secret has already been created as `CACHIX_TOKEN`; it is the Cachix auth token and must be used instead of `CACHIX_AUTH_TOKEN`.

This task must entirely remove Magic Nix Cache from the image publish workflow. It must also remove FlakeHub/Determinate login touchpoints from that workflow, including the current `DeterminateSystems/nix-installer-action` usage if it is what causes `determinate-nixd` / FlakeHub login failures. Prefer `cachix/install-nix-action` for installing Nix and `cachix/cachix-action` for configuring Cachix. The workflow must pull from and push to Cachix cache `djosh34` using `${{ secrets.CACHIX_TOKEN }}`.

The current failure symptoms to address are GitHub Actions annotations like:

- `Failed to restore: Failed to GetCacheEntryDownloadURL: Rate limited: Failed request: (429) Too Many Requests: rate limit exceeded`
- `You've hit a rate limit, your rate limit will reset in 24 seconds`
- `FlakeHub Login failure: The process '/usr/local/bin/determinate-nixd' failed with exit code 1`

Also bump `actions/download-artifact` from the Node.js 20-backed version currently used in the publish job to the current Node.js 24-compatible version. Do not guess blindly: check the available upstream action version at implementation time and use the newest stable major that supports GitHub's Node.js 24 runtime.

The user was given local Cachix/devenv setup instructions that mention:

```nix
{
  cachix.pull = [ "djosh34" ];
}
```

This task is only about GitHub Actions CI caching/publishing workflow changes unless the existing repo already has `devenv.nix` and it is necessary to keep local development cache configuration consistent. Do not introduce unrelated local setup files just because those generic instructions mentioned `devenv`.
</description>

<acceptance_criteria>
- [ ] `.github/workflows/publish-images.yml` no longer references `DeterminateSystems/magic-nix-cache-action`.
- [ ] `.github/workflows/publish-images.yml` no longer has FlakeHub-specific or Determinate login/cache setup in the Nix build jobs.
- [ ] The Nix build jobs install Nix with a non-Determinate path suitable for Cachix, such as `cachix/install-nix-action`.
- [ ] The Nix build jobs configure Cachix with cache name `djosh34` and auth token `${{ secrets.CACHIX_TOKEN }}`.
- [ ] Any `id-token: write` permission used only for Determinate/FlakeHub cache auth is removed from the affected jobs.
- [ ] `actions/download-artifact` is bumped to the current stable Node.js 24-compatible version available when implementing the task.
- [ ] Workflow syntax is validated locally or with an appropriate GitHub Actions validation tool; errors must not be ignored.
- [ ] A real GitHub Actions run of the updated workflow is inspected with authenticated workflow log access, such as `/home/joshazimullah.linux/github-api-curl`, and confirms there are no Magic Nix Cache rate-limit warnings and no FlakeHub login failures.
- [ ] The inspected workflow run confirms Cachix is configured and used for the Nix image build jobs.
- [ ] If Cachix authentication fails, the error is reported directly and a bug task is created rather than being swallowed or worked around silently.
</acceptance_criteria>
