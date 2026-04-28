{
  description = "cockroach_migrate_tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      rust-overlay,
      advisory-db,
      ...
    }:
    flake-utils.lib.eachSystem [
      "x86_64-linux"
      "aarch64-linux"
    ] (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
          config.allowUnfreePredicate = pkg: nixpkgs.lib.getName pkg == "cockroachdb";
        };

        inherit (pkgs) lib;

        rustMuslTarget = {
          x86_64-linux = "x86_64-unknown-linux-musl";
          aarch64-linux = "aarch64-unknown-linux-musl";
        }.${system};

        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p:
          p.rust-bin.stable.latest.default.override {
            targets = [ rustMuslTarget ];
          }
        );
        cockroachdbRuntime =
          let
            version = "23.1.30";
            srcs = {
              aarch64-linux = pkgs.fetchzip {
                url = "https://binaries.cockroachdb.com/cockroach-v${version}.linux-arm64.tgz";
                hash = "sha256-XqyZFm+PI47BrqA3Cu6+2mQxq+9VNHjpAiY0oQdVewU=";
              };
              x86_64-linux = pkgs.fetchzip {
                url = "https://binaries.cockroachdb.com/cockroach-v${version}.linux-amd64.tgz";
                hash = "sha256-LCvwb3+BKhP8PMo/WatN20zvMMaCA3Z6JUVxoJKqNKs=";
              };
            };
            src =
              srcs.${system}
                or (throw "Unsupported system for CockroachDB runtime: ${system}");
          in
          pkgs.buildFHSEnv {
            pname = "cockroachdb";
            inherit version;

            runScript = "${src}/cockroach";

            extraInstallCommands = ''
              cp -P $out/bin/cockroachdb $out/bin/cockroach
            '';

            meta = {
              homepage = "https://www.cockroachlabs.com";
              description = "Scalable, survivable, strongly-consistent SQL database";
              license = with lib.licenses; [
                bsl11
                mit
                cockroachdb-community-license
              ];
              sourceProvenance = with lib.sourceTypes; [ binaryNativeCode ];
              platforms = [
                "aarch64-linux"
                "x86_64-linux"
              ];
            };
          };

        cleanCargoSourceWith =
          extraFilters: source:
          lib.cleanSourceWith {
            src = lib.cleanSource source;
            filter = path: type: (craneLib.filterCargoSources path type) || lib.any (filter: filter path type) extraFilters;
          };
        cleanSelectedSourceWith =
          prefixes: source:
          let
            root = toString source;
          in
          lib.cleanSourceWith {
            src = lib.cleanSource source;
            filter =
              path: _type:
              let
                pathString = toString path;
              in
              lib.any (prefix: lib.hasPrefix prefix pathString) (
                map (prefix: "${root}/${prefix}") prefixes
              );
          };
        src = cleanCargoSourceWith [ ] ./.;
        testSrc = cleanCargoSourceWith [
          (path: _type: lib.hasInfix "/crates/runner/tests/fixtures/" path)
          (path: _type: lib.hasInfix "/investigations/cockroach-webhook-cdc/certs/" path)
        ] ./.;
        moltSrc = lib.cleanSource ./cockroachdb_molt/molt;
        moltTestSrc = cleanSelectedSourceWith [
          "cockroachdb_molt"
          "crates"
          "crates/runner"
          "crates/runner/tests"
          "crates/runner/tests/fixtures"
          "crates/runner/tests/fixtures/certs"
          "investigations"
          "investigations/cockroach-webhook-cdc"
          "investigations/cockroach-webhook-cdc/certs"
        ] ./.;

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          pname = "runner";
          version = "0.1.0";
          strictDeps = true;
          CARGO_BUILD_TARGET = rustMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";

          buildInputs = [
            # Add additional build inputs here
          ];

          nativeBuildInputs = [ ];
        };

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        runner-crate = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;
          }
        );

        # Only build the tests here via doCheck = false
        runner-crate-nextest-build = craneLib.cargoNextest (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          }
        );

        runner-crate-nextest = craneLib.cargoNextest (
          commonArgs
          // {
            src = testSrc;
            nativeBuildInputs = [
              pkgs.openssl
              pkgs.postgresql_16
            ];
            cargoArtifacts = runner-crate-nextest-build;
            doCheck = true;
          }
        );

        runner-crate-nextest-long = craneLib.cargoNextest (
          commonArgs
          // {
            src = testSrc;
            cargoArtifacts = runner-crate-nextest-build;
            nativeBuildInputs = [
              cockroachdbRuntime
              verify-binary
              pkgs.openssl
              pkgs.postgresql_16
            ];
            COCKROACH_BIN = "${cockroachdbRuntime}/bin/cockroach";
            cargoNextestPartitionsExtraArgs = "--run-ignored ignored-only --no-tests=fail";
          }
        );

        verify-binary = pkgs.buildGoModule {
          pname = "verify-binary";
          version = "0.1.4";
          src = moltSrc;
          vendorHash = "sha256-KFDOKXP+Q5fxR4lKWfE2j4V5Vjm+u3tjJbTW2cA8s54=";
          subPackages = [ "." ];
          nativeBuildInputs = [ pkgs.binutils ];
          env.CGO_ENABLED = "0";
          tags = [
            "netgo"
            "osusergo"
          ];
          ldflags = [
            "-s"
            "-w"
          ];

          postInstall = ''
            if readelf -l "$out/bin/molt" | grep -q 'Requesting program interpreter'; then
              echo "verify-binary must be a pure Go static binary without glibc or musl" >&2
              readelf -l "$out/bin/molt" >&2
              exit 1
            fi
          '';
        };

        verify-image = pkgs.dockerTools.buildImage {
          name = "verify-image";
          tag = verify-binary.version;
          config = {
            User = "1000:1000";
            Entrypoint = [ "${verify-binary}/bin/molt" ];
            Cmd = [ "verify-service" ];
          };
        };

        runner-image = pkgs.dockerTools.buildImage {
          name = "runner-image";
          tag = runner-crate.version;
          copyToRoot = runner-crate;
          uid = 1000;
          gid = 1000;
          config = {
            User = "1000:1000";
            Entrypoint = [ "/bin/runner" ];
          };
        };

        molt-go-test-harness = pkgs.writeShellApplication {
          name = "molt-go-test-harness";
          runtimeInputs = [
            cockroachdbRuntime
            pkgs.glibcLocales
            pkgs.postgresql_16
          ];
          text = ''
            set -euo pipefail

            export LOCALE_ARCHIVE="${pkgs.glibcLocales}/lib/locale/locale-archive"
            export LANG="en_US.UTF-8"
            export LC_ALL="en_US.UTF-8"

            tmp_root="$(mktemp -d)"
            pg_data_dir="$tmp_root/postgres"
            crdb_source_store="$tmp_root/cockroach-source"
            crdb_target_store="$tmp_root/cockroach-target"
            pg_log="$tmp_root/postgres.log"
            crdb_source_log="$tmp_root/cockroach-source.log"
            crdb_target_log="$tmp_root/cockroach-target.log"

            cleanup() {
              for pid in "''${cockroach_target_pid:-}" "''${cockroach_source_pid:-}" "''${postgres_pid:-}"; do
                if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
                  kill "$pid" 2>/dev/null || true
                  wait "$pid" 2>/dev/null || true
                fi
              done
              rm -rf "$tmp_root"
            }
            trap cleanup EXIT

            initdb \
              --auth-local=trust \
              --auth-host=trust \
              --username=postgres \
              --encoding=UTF8 \
              --locale=en_US.UTF-8 \
              --pgdata "$pg_data_dir" \
              >"$pg_log" 2>&1

            postgres \
              -D "$pg_data_dir" \
              -F \
              -h 127.0.0.1 \
              -k "$pg_data_dir" \
              -p 5432 \
              -c logging_collector=off \
              >"$pg_log" 2>&1 &
            postgres_pid=$!

            for _ in $(seq 1 60); do
              if pg_isready -h 127.0.0.1 -p 5432 -U postgres -d postgres >/dev/null 2>&1; then
                break
              fi
              if ! kill -0 "$postgres_pid" 2>/dev/null; then
                cat "$pg_log" >&2
                echo "postgres exited before becoming ready" >&2
                exit 1
              fi
              sleep 1
            done
            pg_isready -h 127.0.0.1 -p 5432 -U postgres -d postgres >/dev/null 2>&1 || {
              cat "$pg_log" >&2
              echo "postgres did not become ready" >&2
              exit 1
            }
            psql \
              -h 127.0.0.1 \
              -p 5432 \
              -U postgres \
              -d postgres \
              -v ON_ERROR_STOP=1 \
              -c "CREATE DATABASE defaultdb;" \
              >/dev/null

            cockroach start-single-node \
              --insecure \
              --store "$crdb_source_store" \
              --listen-addr 127.0.0.1:26257 \
              --http-addr 127.0.0.1:18080 \
              >"$crdb_source_log" 2>&1 &
            cockroach_source_pid=$!

            cockroach start-single-node \
              --insecure \
              --store "$crdb_target_store" \
              --listen-addr 127.0.0.1:26258 \
              --http-addr 127.0.0.1:18081 \
              >"$crdb_target_log" 2>&1 &
            cockroach_target_pid=$!

            for _ in $(seq 1 60); do
              source_ready=0
              target_ready=0
              if cockroach sql --insecure --host=127.0.0.1:26257 -e "select 1" >/dev/null 2>&1; then
                source_ready=1
              fi
              if cockroach sql --insecure --host=127.0.0.1:26258 -e "select 1" >/dev/null 2>&1; then
                target_ready=1
              fi
              if [ "$source_ready" -eq 1 ] && [ "$target_ready" -eq 1 ]; then
                break
              fi
              if ! kill -0 "$cockroach_source_pid" 2>/dev/null; then
                cat "$crdb_source_log" >&2
                echo "source cockroach exited before becoming ready" >&2
                exit 1
              fi
              if ! kill -0 "$cockroach_target_pid" 2>/dev/null; then
                cat "$crdb_target_log" >&2
                echo "target cockroach exited before becoming ready" >&2
                exit 1
              fi
              sleep 1
            done

            cockroach sql --insecure --host=127.0.0.1:26257 -e "select 1" >/dev/null 2>&1 || {
              cat "$crdb_source_log" >&2
              echo "source cockroach did not become ready" >&2
              exit 1
            }
            cockroach sql --insecure --host=127.0.0.1:26258 -e "select 1" >/dev/null 2>&1 || {
              cat "$crdb_target_log" >&2
              echo "target cockroach did not become ready" >&2
              exit 1
            }

            export POSTGRES_URL="postgres://postgres:postgres@127.0.0.1:5432/defaultdb"
            export COCKROACH_URL="postgres://root@127.0.0.1:26257/defaultdb?sslmode=disable"
            export COCKROACH_TARGET_URL="postgres://root@127.0.0.1:26258/defaultdb?sslmode=disable"

            go test ./...
          '';
        };

        molt-go-test = pkgs.buildGoModule {
          pname = "molt-go-test";
          version = "0.1.4";
          src = moltTestSrc;
          modRoot = "cockroachdb_molt/molt";
          vendorHash = "sha256-KFDOKXP+Q5fxR4lKWfE2j4V5Vjm+u3tjJbTW2cA8s54=";
          subPackages = [ "." ];
          nativeBuildInputs = [ pkgs.binutils ];
          env.CGO_ENABLED = "0";
          tags = [
            "netgo"
            "osusergo"
          ];
          ldflags = [
            "-s"
            "-w"
          ];

          checkPhase = ''
            runHook preCheck
            ${molt-go-test-harness}/bin/molt-go-test-harness
            runHook postCheck
          '';
        };

        commandApp =
          name: checkNames:
          flake-utils.lib.mkApp {
            drv = pkgs.writeShellApplication {
              inherit name;
              runtimeInputs = [ pkgs.nix ];
              text = ''
                nix build --print-build-logs ${lib.concatMapStringsSep " " (checkName: ".#checks.${system}.${checkName}") checkNames}
              '';
            };
          };
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit runner-crate;

          # Run clippy (and deny all warnings) on the crate source,
          # again, reusing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          runner-crate-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          # Check formatting
          runner-crate-fmt = craneLib.cargoFmt {
            inherit src;
            pname = "runner";
            version = "0.1.0";
          };

          # Run tests with cargo-nextest. The long lane uses the regular nextest
          # derivation as its cargoArtifacts input so it only runs after the
          # default nextest lane succeeds.
          inherit runner-crate-nextest runner-crate-nextest-long molt-go-test;
          test-runner = runner-crate-nextest;
          test-molt = molt-go-test;
          test-long = runner-crate-nextest-long;
        };

        packages = {
          default = runner-crate;
          runner = runner-crate;
          inherit runner-image verify-binary verify-image;
        };

        apps = {
          default = (flake-utils.lib.mkApp {
            drv = runner-crate;
          }) // {
            meta = {
              description = "CockroachDB to PostgreSQL migration runner";
            };
          };

          check = commandApp "check" [
            "runner-crate"
            "runner-crate-clippy"
            "runner-crate-fmt"
            "test-runner"
            "test-molt"
          ];
          lint = commandApp "lint" [
            "runner-crate-clippy"
            "runner-crate-fmt"
          ];
          test = commandApp "test" [
            "test-runner"
            "test-molt"
          ];
          test-runner = commandApp "test-runner" [ "test-runner" ];
          test-molt = commandApp "test-molt" [ "test-molt" ];
          test-long = commandApp "test-long" [ "test-long" ];
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          COCKROACH_BIN = "${cockroachdbRuntime}/bin/cockroach";

          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = [
            # pkgs.ripgrep
          ];
        };
      }
    );
}
