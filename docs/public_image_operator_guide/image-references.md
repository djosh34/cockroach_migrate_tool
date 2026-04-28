# Image References

## Primary registry — GHCR

Images are published to the **GitHub Container Registry (GHCR)** on every push to the repository. The tag is the full Git commit SHA.

### Pull commands

```bash
# Runner
docker pull ghcr.io/djosh34/runner-image:<git-sha>

# Verify-service
docker pull ghcr.io/djosh34/verify-image:<git-sha>
```

Replace `<git-sha>` with the full 40-character commit SHA of the version you want to deploy. Obtain a valid published SHA from the GHCR package pages:

- Runner: `https://github.com/djosh34/cockroach_migrate_tool/pkgs/container/runner-image` — click **View all tagged versions** to see available SHAs.
- Verify: `https://github.com/djosh34/cockroach_migrate_tool/pkgs/container/verify-image` — click **View all tagged versions** to see available SHAs.

Both images are multi-platform manifests supporting `linux/amd64` and `linux/arm64`.

## Quay mirror

Images are also mirrored to Quay, but **GHCR is the source of truth**. Quay mirrors lag behind GHCR and should not be used to determine availability.

```
quay.io/<quay-organization>/<runner-repository>:<git-sha>
quay.io/<quay-organization>/<verify-repository>:<git-sha>
```

The repository names are determined by the CI variables `RUNNER_IMAGE_REPOSITORY` and `VERIFY_IMAGE_REPOSITORY`. The paths shown in GHCR (`runner-image`, `verify-image`) are examples, not guaranteed Quay paths. Consult your organization's CI configuration for the exact repository names.

## Authenticating to GHCR

```bash
echo "$GITHUB_TOKEN" | docker login ghcr.io -u "$GITHUB_USERNAME" --password-stdin
```

The token needs the `read:packages` scope.

## Running a container

Both images default to running their respective subcommands. Pass arguments after the image name:

```bash
# Runner: validate config (offline)
docker run --rm \
  -v ./config:/config:ro \
  ghcr.io/djosh34/runner-image:<git-sha> \
  validate-config --config /config/runner.yml

# Runner: validate config (deep — checks destination connectivity)
docker run --rm \
  -v ./config:/config:ro \
  --network host \
  ghcr.io/djosh34/runner-image:<git-sha> \
  validate-config --config /config/runner.yml --deep

# Runner: start the service
docker run --rm \
  -v ./config:/config:ro \
  -p 8443:8443 \
  ghcr.io/djosh34/runner-image:<git-sha> \
  run --config /config/runner.yml

# Verify-service: validate config
docker run --rm \
  -v ./config:/config:ro \
  ghcr.io/djosh34/verify-image:<git-sha> \
  verify-service validate-config --config /config/verify-service.yml

# Verify-service: start the service
docker run --rm \
  -v ./config:/config:ro \
  -p 8080:8080 \
  ghcr.io/djosh34/verify-image:<git-sha> \
  verify-service run --config /config/verify-service.yml
```

> **Note:** The verify image's entrypoint is `molt` and its default command is `verify-service`. Always include the `verify-service` subcommand explicitly — especially when overriding `command` in Docker Compose.

## Log format

Both images support structured JSON logging via `--log-format`. The flag is placed differently depending on the image:

| Value | Behavior |
| ----- | -------- |
| `text` | Human-readable console output (default) |
| `json` | Structured JSON for log aggregators |

- **Runner:** `--log-format` is a global flag that precedes the subcommand:

```bash
docker run --rm \
  ghcr.io/djosh34/runner-image:<git-sha> \
  --log-format json \
  validate-config --config /config/runner.yml
```

- **Verify-service:** `--log-format` is a flag on the `validate-config` and `run` subcommands:

```bash
docker run --rm \
  ghcr.io/djosh34/verify-image:<git-sha> \
  verify-service validate-config --log-format json --config /config/verify-service.yml
```

## See also

- [Runner getting started](runner/getting-started.md) — full runner setup walkthrough
- [Verify getting started](verify/getting-started.md) — full verify-service setup walkthrough
