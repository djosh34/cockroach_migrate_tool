You are `opencode-go/glm-5.1`, still the sole documentation author for this task.

Read and edit only files inside:

- `docs/public_image_operator_guide/`

Goals for this remediation pass:

- Fix the independent-review blockers and ambiguities.
- Keep the docs self-contained for an outside operator using only the published images.
- Preserve your authorship of the prose.
- Edit the files in place.

Verified facts you must use:

1. Published image discovery without repo access

- The public GitHub package pages are reachable and show the install command plus recent tagged versions.
- Verified runner page:
  - `https://github.com/djosh34/cockroach_migrate_tool/pkgs/container/runner-image`
- Verified verify page:
  - `https://github.com/djosh34/cockroach_migrate_tool/pkgs/container/verify-image`
- The runner package page shows:
  - the current install command
  - recent tagged image versions
  - `View all tagged versions`
- Do not hardcode the current latest SHA as the recommended deployment target.
- You may instruct operators to obtain a valid published commit SHA from those package pages.

2. Image publication scope

- `.github/workflows/publish-images.yml` currently runs on `push` with no branch filter.
- Do not say images are published only from the default branch.

3. Quay mirror naming

- Quay refs are built from `QUAY_ORGANIZATION` plus the CI variables `RUNNER_IMAGE_REPOSITORY` and `VERIFY_IMAGE_REPOSITORY`.
- Do not hardcode Quay repository names as `runner-image` and `verify-image` unless you explicitly say they are examples rather than verified paths.

4. Verify-service CLI flag order

- In `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`, `--log-format` is registered on the `validate-config` and `run` subcommands.
- Supported forms:
  - `verify-service validate-config --log-format json --config /config/verify-service.yml`
  - `verify-service run --log-format json --config /config/verify-service.yml`
- Do not document `verify-service --log-format json validate-config ...` or `verify-service --log-format json run ...`.

5. Verify-image container contract

- `flake.nix` sets:
  - `Entrypoint = [ ".../bin/molt" ]`
  - `Cmd = [ "verify-service" ]`
- When Compose overrides `command`, it replaces the default `Cmd`, so examples must preserve the `verify-service` subcommand explicitly.

6. Verify HTTPS port mapping

- If the example config uses `listener.bind_addr: 0.0.0.0:8443`, the container port mapping must target `8443`, not `8080`.

7. Source setup ordering

- `README.md` says: start the runtime after source changefeeds and destination grants are already in place.
- The source setup docs should not present the opposite order as the main bootstrap path unless they clearly label it as an alternative and reconcile the contradiction.

8. Source setup trailing slash behavior

- `scripts/generate-cockroach-setup-sql.sh` trims trailing slashes from `webhook.base_url` before appending `/ingest/<mapping_id>`.
- Do not present a trailing slash as a hard failure if the current implementation normalizes it.

9. `ca_cert` placeholder wording

- The sink URL needs percent-encoded base64 certificate data, not raw base64.

Independent-review findings you must address:

- The docs were judged insufficient because an outside operator had no source-backed way to discover a usable published image tag without repo access.
- The verify-service Compose example would likely execute `molt run ...` instead of `molt verify-service run ...`.
- The verify-service Compose example mixed host port `9443` with container port `8080` while the HTTPS example above used bind address `8443`.
- The docs contained conflicting `--log-format` invocation patterns.
- The source-setup order conflicted with the top-level bootstrap contract.

Comment handling:

- If a nearby inline `<comment>...</comment>` block is fully resolved by your revision, remove it.
- If you cannot honestly resolve a comment with source-backed prose, leave it in place.

Do this remediation pass now.
