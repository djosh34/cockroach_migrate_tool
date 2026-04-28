{
  description = "Crane-backed local workflows for cockroach_migrate_tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      crane,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        runnerManifest = builtins.fromTOML (builtins.readFile ./crates/runner/Cargo.toml);
        runnerPname = runnerManifest.package.name;
        runnerVersion = runnerManifest.package.version;
        runnerMuslTarget =
          {
            x86_64-linux = "x86_64-unknown-linux-musl";
            aarch64-linux = "aarch64-unknown-linux-musl";
          }
          .${system};
        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p:
          p.rust-bin.stable.latest.default.override {
            targets = [ runnerMuslTarget ];
          }
        );
        cargoSrc = craneLib.cleanCargoSource ./.;
        repoSrc = pkgs.lib.cleanSource ./.;
        goVersion = "0.1.0";
        runnerTestInputs = [
          pkgs.gettext
          pkgs.openssl
          pkgs.postgresql
          pkgs.python3
          pkgs.yq-go
        ];
        mkNixBuildApp =
          name: targets:
          pkgs.writeShellApplication {
            inherit name;
            runtimeInputs = [ pkgs.nix ];
            text = ''
              exec nix build --no-link ${targets}
            '';
          };

        runnerCargoArtifacts = craneLib.buildDepsOnly {
          pname = "${runnerPname}-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          doCheck = false;
          cargoExtraArgs = "-p runner --locked";
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        };

        runner = craneLib.buildPackage {
          cargoArtifacts = runnerCargoArtifacts;
          pname = runnerPname;
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoExtraArgs = "-p runner";
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          doCheck = false;
        };

        runnerLintCargoArtifacts = craneLib.buildDepsOnly {
          pname = "${runnerPname}-lint-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          doCheck = false;
          cargoExtraArgs = "--workspace --locked";
          cargoCheckExtraArgs = "--all-targets";
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        };

        runnerLint = craneLib.cargoClippy {
          cargoArtifacts = runnerLintCargoArtifacts;
          pname = "${runnerPname}-lint";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoClippyExtraArgs = "--workspace --all-targets -- -D warnings";
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        };

        runnerTestCargoArtifacts = craneLib.buildDepsOnly {
          pname = "${runnerPname}-test-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoExtraArgs = "--workspace --locked";
          cargoCheckExtraArgs = "--all-targets";
          cargoTestExtraArgs = "--no-run";
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        };

        runnerTest = craneLib.cargoTest {
          cargoArtifacts = runnerTestCargoArtifacts;
          pname = "${runnerPname}-test";
          version = runnerVersion;
          src = repoSrc;
          strictDeps = true;
          nativeBuildInputs = runnerTestInputs;
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          preCheck = ''
            patchShebangs scripts
          '';
          cargoTestExtraArgs = "--workspace";
        };

        runnerTestLong = craneLib.cargoTest {
          cargoArtifacts = runnerTestCargoArtifacts;
          pname = "${runnerPname}-test-long";
          version = runnerVersion;
          src = repoSrc;
          strictDeps = true;
          nativeBuildInputs = runnerTestInputs;
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          preCheck = ''
            patchShebangs scripts
          '';
          cargoTestExtraArgs = "--workspace -- --ignored --test-threads=1";
        };

        runnerFmt = craneLib.cargoFmt {
          pname = "${runnerPname}-fmt";
          version = runnerVersion;
          src = cargoSrc;
          cargoFmtExtraArgs = "--all --check";
        };

        verify = pkgs.pkgsStatic.buildGoModule {
          pname = "verify";
          version = goVersion;
          src = ./.;
          modRoot = "cockroachdb_molt/molt";
          vendorHash = "sha256-7yHLVPjLmZxUbe9MxCzK3jqIPWEV27XKFQl/0yDgt4o=";
          subPackages = [ "." ];
          nativeBuildInputs = [ pkgs.removeReferencesTo ];
          postFixup = ''
            remove-references-to \
              -t ${pkgs.tzdata} \
              -t ${pkgs.mailcap} \
              -t ${pkgs.iana-etc} \
              "$out/bin/molt"
          '';
          doCheck = false;
        };

        verifyService = pkgs.writeShellApplication {
          name = "verify-service";
          text = ''
            exec ${verify}/bin/molt verify-service "$@"
          '';
        };

        verifyTest = pkgs.buildGoModule {
          pname = "verify-test";
          version = goVersion;
          src = ./.;
          modRoot = "cockroachdb_molt/molt";
          vendorHash = "sha256-7yHLVPjLmZxUbe9MxCzK3jqIPWEV27XKFQl/0yDgt4o=";
          subPackages = [ "." ];
          doCheck = true;
          checkPhase = ''
            runHook preCheck
            export HOME="$TMPDIR/home"
            mkdir -p "$HOME"
            go test ./cmd/verifyservice -count=1
            runHook postCheck
          '';
        };

        mkRuntimeImage =
          {
            imageName,
            imageTag,
            binaryPath,
            entrypoint,
          }:
          pkgs.dockerTools.buildImage {
            name = imageName;
            tag = imageTag;
            extraCommands = ''
              mkdir -p usr/local/bin
              cp ${binaryPath} usr/local/bin/$(basename ${binaryPath})
              chmod 0555 usr/local/bin/$(basename ${binaryPath})
            '';
            config = {
              Entrypoint = entrypoint;
            };
          };

        runnerImage = mkRuntimeImage {
          imageName = "cockroach-migrate-runner";
          imageTag = "nix";
          binaryPath = "${runner}/bin/runner";
          entrypoint = [ "/usr/local/bin/runner" ];
        };

        verifyImage = mkRuntimeImage {
          imageName = "cockroach-migrate-verify";
          imageTag = "nix";
          binaryPath = "${verify}/bin/molt";
          entrypoint = [
            "/usr/local/bin/molt"
            "verify-service"
          ];
        };

        fmtApp = mkNixBuildApp "fmt" ".#checks.${system}.runner-fmt";
        lintApp = mkNixBuildApp "lint" ".#checks.${system}.runner-lint";
        testApp = mkNixBuildApp "test" ".#checks.${system}.runner-test .#checks.${system}.verify-test";
        longTestApp = mkNixBuildApp "test-long" ".#checks.${system}.runner-test-long";
        checkApp = mkNixBuildApp "check" ".#checks.${system}.runner-fmt .#checks.${system}.runner-lint .#checks.${system}.runner-test .#checks.${system}.verify-test";
      in
      {
        packages = {
          default = runner;
          runner = runner;
          "runner-image" = runnerImage;
          verify = verify;
          "verify-image" = verifyImage;
          "verify-service" = verifyService;
          fmt = fmtApp;
          lint = lintApp;
          test = testApp;
          "test-long" = longTestApp;
          check = checkApp;
        };

        apps = {
          runner = flake-utils.lib.mkApp {
            drv = runner;
            exePath = "/bin/runner";
          };
          "verify-service" = flake-utils.lib.mkApp { drv = verifyService; };
          fmt = flake-utils.lib.mkApp { drv = fmtApp; };
          lint = flake-utils.lib.mkApp { drv = lintApp; };
          test = flake-utils.lib.mkApp { drv = testApp; };
          "test-long" = flake-utils.lib.mkApp { drv = longTestApp; };
          check = flake-utils.lib.mkApp { drv = checkApp; };
        };

        checks = {
          runner-fmt = runnerFmt;
          runner-lint = runnerLint;
          runner-test = runnerTest;
          runner-test-long = runnerTestLong;
          verify-test = verifyTest;
        };

        devShells.default = pkgs.mkShell {
          packages = runnerTestInputs ++ [
            pkgs.cargo
            pkgs.clippy
            pkgs.go_1_26
            pkgs.rustc
            pkgs.rustfmt
          ];
        };

        formatter = pkgs.nixfmt;
      }
    );
}
