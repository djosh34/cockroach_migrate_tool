## Smell Set: 2026-04-28-story-29-ci-to-nix-boundaries <status>completed</status> <passes>true</passes>

Please refer to skill 'improve-code-boundaries' to see what smells there are.

Inside dirs:
- `.github/workflows`
- `.`

Solve each smell:

---
- [x] Smell 3, Wrong Place-ism
The repository flake now owns the runtime image build graph, but the GitHub workflow catalog still owns image identity, Dockerfile paths, build contexts, cache scopes, and publish metadata. That leaves the workflow as a second packaging authority and forces CI to remember implementation details that should live next to the Nix image outputs.

code:
```nix
runnerImage = mkRuntimeImage {
  imageName = "cockroach-migrate-runner";
  imageTag = "nix";
  binaryPath = "${runnerRuntime}/bin/runner";
  entrypoint = [ "/usr/local/bin/runner" ];
};

verifyImage = mkRuntimeImage {
  imageName = "cockroach-migrate-verify";
  imageTag = "nix";
  binaryPath = "${moltRuntime}/bin/molt";
  entrypoint = [
    "/usr/local/bin/molt"
    "verify-service"
  ];
};
```

```yaml
[
  {
    "image_id": "runner",
    "quay_repository": "runner",
    "ghcr_repository": "cockroach-migrate-runner",
    "dockerfile": "./Dockerfile",
    "context": ".",
    "manifest_key": "runner_image_ref",
    "artifact_name": "published-image-runner",
    "cache_scope": "publish-image-runner",
    "build_kind": "rust-workspace-musl"
  },
  {
    "image_id": "verify",
    "quay_repository": "verify",
    "ghcr_repository": "cockroach-migrate-verify",
    "dockerfile": "./cockroachdb_molt/molt/Dockerfile",
    "context": "./cockroachdb_molt/molt",
    "manifest_key": "verify_image_ref",
    "artifact_name": "published-image-verify",
    "cache_scope": "publish-image-verify",
    "build_kind": "verify-go"
  }
]
```

```yaml
docker buildx build \
  --progress plain \
  --platform "${{ matrix.platform.platform }}" \
  --file "${{ matrix.image.dockerfile }}" \
  --cache-from "type=gha,scope=${{ matrix.image.cache_scope }}-${{ matrix.platform.platform_tag_suffix }}" \
  --cache-to "type=gha,scope=${{ matrix.image.cache_scope }}-${{ matrix.platform.platform_tag_suffix }},mode=max" \
```

---
- [x] Smell 4, Display Not Strings
The workflow emits structured publish metadata by hand-building JSON and env files inside shell heredocs. That makes the CI contract stringly and brittle, and it duplicates typed information that could be emitted from a single structured Nix surface instead of being hand-rendered in YAML.

code:
```yaml
release_image_catalog="$(cat <<'EOF'
[
  {
    "image_id": "runner",
    "quay_repository": "runner",
    "ghcr_repository": "cockroach-migrate-runner",
    "dockerfile": "./Dockerfile",
    "context": ".",
    "manifest_key": "runner_image_ref",
    "artifact_name": "published-image-runner",
    "cache_scope": "publish-image-runner",
    "build_kind": "rust-workspace-musl"
  },
  {
    "image_id": "verify",
    "quay_repository": "verify",
    "ghcr_repository": "cockroach-migrate-verify",
    "dockerfile": "./cockroachdb_molt/molt/Dockerfile",
    "context": "./cockroachdb_molt/molt",
    "manifest_key": "verify_image_ref",
    "artifact_name": "published-image-verify",
    "cache_scope": "publish-image-verify",
    "build_kind": "verify-go"
  }
]
EOF
)"
```

```yaml
{
  echo "release_image_catalog<<EOF"
  echo "${release_image_catalog}"
  echo "EOF"
  echo "publish_image_matrix<<EOF"
  echo "${publish_image_matrix}"
  echo "EOF"
} >> "${GITHUB_OUTPUT}"
```

```yaml
{
  printf '{\n'
  for index in "${!json_entries[@]}"; do
    if [[ "${index}" -gt 0 ]]; then
      printf ',\n'
    fi
    printf '%s' "${json_entries[${index}]}"
  done
  printf '\n}\n'
} > "${{ env.PUBLISHED_IMAGE_MANIFEST }}"
```

---
- [x] Smell 13, Multiple Functions With Large Overlap
The workflow repeats the same setup boundary across validate jobs and publish jobs: checkout, system package install, Rust or Docker/Buildx/Nix bootstrap, then lane execution. The duplication makes CI policy changes fan out across several jobs instead of one honest setup boundary. After the Nix migration, the shared setup should collapse around one Nix bootstrap plus job-specific command surfaces rather than multiple near-copies.

code:
```yaml
validate-fast:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v6
    - name: Restore Cargo registry cache
      uses: actions/cache/restore@v5
    - name: Restore Cargo target cache
      uses: actions/cache/restore@v5
    - name: Install Rust toolchain
      run: |
        set -euo pipefail
        sudo apt-get update
        sudo apt-get install --yes postgresql
        rustup toolchain install 1.93.0 --profile minimal --component clippy
        rustup default 1.93.0
    - name: Validate fast repository lanes
      run: |
        set -euo pipefail
        make check
        make lint
        make test
```

```yaml
validate-long:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v6
    - name: Restore Cargo registry cache
      uses: actions/cache/restore@v5
    - name: Restore Cargo target cache
      uses: actions/cache/restore@v5
    - name: Install Rust toolchain
      run: |
        set -euo pipefail
        sudo apt-get update
        sudo apt-get install --yes postgresql
        rustup toolchain install 1.93.0 --profile minimal --component clippy
        rustup default 1.93.0
    - name: Validate ultra-long repository lane
      run: |
        set -euo pipefail
        make test-long
```

```yaml
- name: Install publish dependencies
  run: |
    set -euo pipefail
    sudo apt-get update
    BUILDX_VERSION=v0.30.1
    case "${{ runner.arch }}" in
      X64)
        buildx_arch=amd64
        ;;
      ARM64)
        buildx_arch=arm64
        ;;
      *)
        printf 'unsupported runner.arch for buildx install: %s\n' "${{ runner.arch }}" >&2
        exit 1
        ;;
    esac
    mkdir -p "${HOME}/.docker/cli-plugins"
    curl -fsSL "https://github.com/docker/buildx/releases/download/${BUILDX_VERSION}/buildx-${BUILDX_VERSION}.linux-${buildx_arch}" \
      -o "${HOME}/.docker/cli-plugins/docker-buildx"
    chmod +x "${HOME}/.docker/cli-plugins/docker-buildx"
docker buildx version
```

---

Resolved by:
- moving static publish metadata into `flake.nix` as `github.publishImageCatalog`
- deleting the workflow-owned image catalog boundary and consuming `nix eval --json` outputs instead
- replacing Dockerfile rebuild lanes with Nix-built image archives published through `skopeo`
- collapsing hosted validation and publish bootstrap around Nix installs plus flake-defined command surfaces
