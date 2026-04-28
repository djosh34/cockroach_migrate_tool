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
    let
      githubPublishPlatforms = [
        {
          system = "x86_64-linux";
          platform = "linux/amd64";
          platform_tag_suffix = "amd64";
          runner = "ubuntu-24.04";
        }
        {
          system = "aarch64-linux";
          platform = "linux/arm64";
          platform_tag_suffix = "arm64";
          runner = "ubuntu-24.04-arm";
        }
      ];
      githubPublishImages = [
        {
          artifact_name = "published-image-runner";
          ghcr_repository = "cockroach-migrate-runner";
          image_id = "runner";
          loaded_image_ref = "cockroach-migrate-runner:nix";
          manifest_key = "runner_image_ref";
          package_attr = "runner-image";
          quay_repository = "runner";
        }
        {
          artifact_name = "published-image-verify";
          ghcr_repository = "cockroach-migrate-verify";
          image_id = "verify";
          loaded_image_ref = "cockroach-migrate-verify:nix";
          manifest_key = "verify_image_ref";
          package_attr = "verify-image";
          quay_repository = "verify";
        }
      ];
      githubPublishImageMatrix = {
        include = nixpkgs.lib.concatMap (
          image:
          map (platform: {
            inherit image platform;
          }) githubPublishPlatforms
        ) githubPublishImages;
      };
    in
    (flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        craneLib = crane.mkLib pkgs;
        cargoSrc = craneLib.cleanCargoSource ./.;
        repoSrc = pkgs.lib.cleanSource ./.;
        runnerManifest = builtins.fromTOML (builtins.readFile ./crates/runner/Cargo.toml);
        runnerPname = runnerManifest.package.name;
        runnerVersion = runnerManifest.package.version;
        runnerMuslTarget =
          {
            x86_64-linux = "x86_64-unknown-linux-musl";
            aarch64-linux = "aarch64-unknown-linux-musl";
          }
          .${system};
        craneLibMusl = (crane.mkLib pkgs).overrideToolchain (
          p:
          p.rust-bin.stable.latest.default.override {
            targets = [ runnerMuslTarget ];
          }
        );
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

        cargoArtifacts = craneLib.buildDepsOnly {
          pname = "${runnerPname}-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
        };

        runner = craneLib.buildPackage {
          inherit cargoArtifacts;
          pname = runnerPname;
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoExtraArgs = "-p runner";
          doCheck = false;
        };

        runnerRuntimeCargoArtifacts = craneLibMusl.buildDepsOnly {
          pname = "${runnerPname}-runtime-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        };

        runnerRuntime = craneLibMusl.buildPackage {
          cargoArtifacts = runnerRuntimeCargoArtifacts;
          pname = "${runnerPname}-runtime";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoExtraArgs = "-p runner";
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          doCheck = false;
        };

        runnerClippy = craneLib.cargoClippy {
          inherit cargoArtifacts;
          pname = "${runnerPname}-clippy";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoClippyExtraArgs = "--workspace --all-targets -- -D warnings";
        };

        runnerTest = craneLib.cargoTest {
          inherit cargoArtifacts;
          pname = "${runnerPname}-test";
          version = runnerVersion;
          src = repoSrc;
          strictDeps = true;
          nativeBuildInputs = runnerTestInputs;
          preCheck = ''
            patchShebangs scripts
          '';
          cargoTestExtraArgs = "--workspace";
        };

        runnerLongTest = craneLib.cargoTest {
          inherit cargoArtifacts;
          pname = "${runnerPname}-long-test";
          version = runnerVersion;
          src = repoSrc;
          strictDeps = true;
          nativeBuildInputs = runnerTestInputs;
          preCheck = ''
            patchShebangs scripts
          '';
          cargoTestExtraArgs = "--workspace -- --ignored --test-threads=1";
        };

        fmtCheck = craneLib.cargoFmt {
          pname = "${runnerPname}-fmt";
          version = runnerVersion;
          src = cargoSrc;
          cargoFmtExtraArgs = "--all --check";
        };

        molt = pkgs.buildGoModule {
          pname = "molt";
          version = goVersion;
          src = ./.;
          modRoot = "cockroachdb_molt/molt";
          vendorHash = "sha256-2imVjNA9cJXqh4JIPoWnShh2quDpZA4ji+PmWT3DeMk=";
          subPackages = [ "." ];
          doCheck = false;
        };

        moltRuntime = pkgs.pkgsStatic.buildGoModule {
          pname = "molt-runtime";
          version = goVersion;
          src = ./.;
          modRoot = "cockroachdb_molt/molt";
          vendorHash = "sha256-2imVjNA9cJXqh4JIPoWnShh2quDpZA4ji+PmWT3DeMk=";
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
            exec ${molt}/bin/molt verify-service "$@"
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

        verifyServiceTest = pkgs.buildGoModule {
          pname = "verify-service-test";
          version = goVersion;
          src = ./.;
          modRoot = "cockroachdb_molt/molt";
          vendorHash = "sha256-2imVjNA9cJXqh4JIPoWnShh2quDpZA4ji+PmWT3DeMk=";
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

        checkApp = mkNixBuildApp "check" ".#checks.${system}.runner-clippy";
        lintApp = mkNixBuildApp "lint" ".#checks.${system}.runner-clippy";
        testApp = mkNixBuildApp "test" ".#checks.${system}.runner-test .#checks.${system}.verify-service-test";
        fmtApp = mkNixBuildApp "fmt" ".#checks.${system}.fmt-check";
        longTestApp = mkNixBuildApp "test-long" ".#packages.${system}.runner-long-test";
      in
      {
        packages = {
          default = runner;
          runner = runner;
          "cargo-artifacts" = cargoArtifacts;
          check = checkApp;
          lint = lintApp;
          test = testApp;
          fmt = fmtApp;
          molt = molt;
          "molt-runtime" = moltRuntime;
          "runner-runtime" = runnerRuntime;
          "runner-image" = runnerImage;
          "runner-long-test" = runnerLongTest;
          "test-long" = longTestApp;
          "verify-image" = verifyImage;
          "verify-service" = verifyService;
        };

        apps = {
          runner = flake-utils.lib.mkApp {
            drv = runner;
            exePath = "/bin/runner";
          };
          "verify-service" = flake-utils.lib.mkApp { drv = verifyService; };
          check = flake-utils.lib.mkApp { drv = checkApp; };
          lint = flake-utils.lib.mkApp { drv = lintApp; };
          test = flake-utils.lib.mkApp { drv = testApp; };
          fmt = flake-utils.lib.mkApp { drv = fmtApp; };
          "test-long" = flake-utils.lib.mkApp { drv = longTestApp; };
        };

        checks = {
          runner-clippy = runnerClippy;
          runner-test = runnerTest;
          verify-service-test = verifyServiceTest;
          fmt-check = fmtCheck;
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

        formatter = pkgs.nixfmt-rfc-style;
      }
    ))
    // {
      github.publishImageCatalog = {
        platforms = githubPublishPlatforms;
        images = githubPublishImages;
        publish_image_matrix = githubPublishImageMatrix;
        release_image_catalog = githubPublishImages;
      };
    };
}
