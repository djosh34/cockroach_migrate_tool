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
        cockroachdb =
          assert lib.hasPrefix "23.1." pkgs.cockroachdb.version;
          pkgs.cockroachdb;

        cleanCargoSourceWith =
          extraFilters: source:
          lib.cleanSourceWith {
            src = lib.cleanSource source;
            filter = path: type: (craneLib.filterCargoSources path type) || lib.any (filter: filter path type) extraFilters;
          };
        src = cleanCargoSourceWith [ ] ./.;
        testSrc = cleanCargoSourceWith [
          (path: _type: lib.hasInfix "/crates/runner/tests/fixtures/" path)
          (path: _type: lib.hasInfix "/investigations/cockroach-webhook-cdc/certs/" path)
        ] ./.;
        moltSrc = lib.cleanSource ./cockroachdb_molt/molt;

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
              cockroachdb
              verify-binary
              pkgs.openssl
              pkgs.postgresql_16
            ];
            COCKROACH_BIN = "${cockroachdb}/bin/cockroach";
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

        molt-go-test = pkgs.buildGoModule {
          pname = "molt-go-test";
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

          checkPhase = ''
            runHook preCheck
            go test ./...
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
          COCKROACH_BIN = "${cockroachdb}/bin/cockroach";

          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = [
            # pkgs.ripgrep
          ];
        };
      }
    );
}
