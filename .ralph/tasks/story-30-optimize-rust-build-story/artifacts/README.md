# Story 30 Dependency Optimization Artifacts

This directory tracks reproducible evidence for story 30 task 01.

## Baseline Pre-Change

Directory:
- `baseline-pre/`

Primary files:
- `cargo-metadata-no-deps.json`
- `cargo-tree-workspace.txt`
- `unique-crate-count.txt`
- `cargo-artifacts-drv-path.json`
- `runner-drv-path.json`
- `runner-runtime-drv-path.json`
- `cargo-artifacts-out-path.txt`
- `cargo-artifacts-path-info.txt`
- `cargo-artifacts-build-time-seconds.txt`

Recorded values:
- workspace unique crate count: `200`
- `cargo-artifacts` output path: `/nix/store/jmhd23yc9inhj3c6yjgs93xc9zk2giqi-runner-deps-deps-0.1.0`
- `cargo-artifacts` size: `525.2 MiB`
- `cargo-artifacts` build time: `1.281s`

Commands used:
- `cargo metadata --format-version 1 --no-deps`
- `cargo tree --workspace --prefix none -e normal,build,dev`
- `nix eval --json .#packages.aarch64-linux.cargo-artifacts.drvPath`
- `nix eval --json .#packages.aarch64-linux.runner.drvPath`
- `nix eval --json .#packages.aarch64-linux.runner-runtime.drvPath`
- `nix build .#cargo-artifacts --no-link --print-out-paths`
- `nix path-info -Sh <cargo-artifacts-output>`
- `bash -lc 'TIMEFORMAT=%R; time nix build .#cargo-artifacts --no-link > /dev/null'`

## Intermediate Current

Directory:
- `intermediate-current/`

Primary files:
- `cargo-metadata-no-deps.json`
- `cargo-tree-workspace.txt`
- `unique-crate-count.txt`
- `cargo-artifacts-drv-path.txt`
- `cargo-artifacts-out-path.txt`
- `cargo-artifacts-path-info.txt`
- `cargo-artifacts-build-time-seconds.txt`

Recorded values after the current in-progress pruning:
- workspace unique crate count: `188`
- `cargo-artifacts` output path: `/nix/store/1395c9mj49vrzqmskirr7sx12y07y35z-runner-deps-deps-0.1.0`
- `cargo-artifacts` size: `521.4 MiB`
- `cargo-artifacts` build time: `1.314s`

Current conclusion:
- The completed cuts removed real dependency leaks and improved the graph, but they do not come close to the story target yet.
- The next material pruning opportunity is likely in dev-dependency tooling, especially `reqwest`, `assert_cmd`, `predicates`, and `tempfile`, or in a deeper crate boundary split between lightweight CLI/config paths and heavy runtime paths.
