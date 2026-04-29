# Installation

Both images are published to the GitHub Container Registry (GHCR) on every push and mirrored to Quay. GHCR is the source of truth.

## Finding the right image tag

Image tags are the full 40-character Git commit SHA from each push. The GHCR package page for each image lists all published tags.

Set `GITHUB_OWNER` to your GitHub repository owner (the organization or user that owns the repository) before running any commands below, or substitute it directly.

**GHCR paths:**

| Image | GHCR path |
|-------|-----------|
| Runner | `ghcr.io/${GITHUB_OWNER}/runner-image` |
| Verify | `ghcr.io/${GITHUB_OWNER}/verify-image` |

To find the latest tag, visit your repository's package pages:

- Runner: `https://github.com/${GITHUB_OWNER}/<repo>/pkgs/container/runner-image`
- Verify: `https://github.com/${GITHUB_OWNER}/<repo>/pkgs/container/verify-image`

Each page shows available tag versions (commit SHAs) and multi-platform digests. Pick the SHA for the commit you want to deploy.

## Pull commands

```bash
export GITHUB_OWNER="<your-github-org-or-user>"
docker pull "ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha>"
docker pull "ghcr.io/${GITHUB_OWNER}/verify-image:<git-sha>"
```

Replace `<git-sha>` with a full 40-character commit SHA from the package page.

## Quay mirror

Images are copied to Quay after each GHCR publish. Quay repository names are determined at build time and may differ from the GHCR names. GHCR is the source of truth — always determine availability from GHCR, not Quay.

```
quay.io/<quay-organization>/<runner-repository>:<git-sha>
quay.io/<quay-organization>/<verify-repository>:<git-sha>
```

## Authentication

```bash
echo "$GITHUB_TOKEN" | docker login ghcr.io -u "$GITHUB_USERNAME" --password-stdin
```

The token requires the `read:packages` scope.

## Running a container

Both images default to their respective subcommand. Pass arguments after the image name.

### Validate runner config (offline)

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha> \
  validate-config --config /config/runner.yml
```

### Validate runner config (deep — tests destination connectivity)

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  --network host \
  ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha> \
  validate-config --config /config/runner.yml --deep
```

### Start the runner

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha> \
  run --config /config/runner.yml
```

### Validate verify-service config

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/${GITHUB_OWNER}/verify-image:<git-sha> \
  verify-service validate-config --config /config/verify-service.yml
```

### Start the verify-service

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/${GITHUB_OWNER}/verify-image:<git-sha> \
  verify-service run --config /config/verify-service.yml
```

> **Entrypoint asymmetry.** The runner and verify images use different entrypoint conventions:
>
> | Image | Entrypoint | Default command | How to override `command` in Compose |
> |-------|-----------|----------------|--------------------------------------|
> | `runner-image` | `runner` (the binary) | *(none — default CMD invokes `runner`)* | Pass positional args directly, e.g. `command: ["run", "--config", "/config/runner.yml"]` |
> | `verify-image` | `molt` | `verify-service` | Always include `verify-service` as the first argument, e.g. `command: ["verify-service", "run", "--config", "/config/verify-service.yml"]` |
>
> This explains why CLI examples treat the two images differently: runner commands start directly with a subcommand (`validate-config`, `run`), while verify commands require the `verify-service` prefix (`verify-service validate-config`, `verify-service run`).

## Log format

Both images support `--log-format text|json`. The flag position differs:

| Image | Flag position | Example |
|-------|--------------|---------|
| `runner-image` | Global flag, before the subcommand | `--log-format json validate-config --config ...` |
| `verify-image` | Flag on the subcommand | `verify-service validate-config --log-format json --config ...` |

## Next steps

- [Set up TLS certificates](tls-configuration.md) — required before writing component configs
- [Getting Started](getting-started.md) — complete end-to-end walkthrough
- [Source & Destination Setup](setup-sql.md) — CockroachDB changefeeds and PostgreSQL grants
