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
          ghcr_repository = "cockroach-migrate-runner";
          image_id = "runner";
          loaded_image_ref = "cockroach-migrate-runner:nix";
          manifest_key = "runner_image_ref";
          package_attr = "runner-image";
          quay_repository = "runner";
        }
        {
          ghcr_repository = "cockroach-migrate-verify";
          image_id = "verify";
          loaded_image_ref = "cockroach-migrate-verify:nix";
          manifest_key = "verify_image_ref";
          package_attr = "verify-image";
          quay_repository = "verify";
        }
      ];
      githubCiBuildPlatforms = map (platform: platform // { installables = [ ".#packages.${platform.system}.ci-build-bundle" ]; }) githubPublishPlatforms;
      githubCiValidationLanes = [
        {
          installables = [
            ".#checks.x86_64-linux.runner-clippy"
            ".#checks.x86_64-linux.runner-test"
            ".#checks.x86_64-linux.verify-service-test"
          ];
          runner = "ubuntu-24.04";
          system = "x86_64-linux";
          validation_id = "fast";
        }
      ];
      githubCiImageMatrix = {
        include = nixpkgs.lib.concatMap (
          image:
          map (platform: {
            inherit image platform;
            image_installable = ".#packages.${platform.system}.${image.package_attr}";
            image_output_id = "${image.image_id}-${platform.platform_tag_suffix}";
          }) githubPublishPlatforms
        ) githubPublishImages;
      };
      githubCiWorkflowCatalog = {
        audit = {
          required_build_systems = map (platform: platform.system) githubCiBuildPlatforms;
          required_validation_ids = map (lane: lane.validation_id) githubCiValidationLanes;
          required_image_output_ids = map (entry: entry.image_output_id) githubCiImageMatrix.include;
          required_publish_output_ids = map (entry: entry.image_output_id) githubCiImageMatrix.include;
        };
        build_platform_matrix = {
          include = githubCiBuildPlatforms;
        };
        image_matrix = githubCiImageMatrix;
        release_image_catalog = githubPublishImages;
        validation_matrix = {
          include = githubCiValidationLanes;
        };
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

        sanitizeCargoArtifacts = ''
          if [ -d target ]; then
            find target -type f \( \
              -path '*/build/*/out/*.a' -o \
              -path '*/build/*/out/*.o' -o \
              -path '*/deps/*.rlib' -o \
              -path '*/deps/*.rmeta' \
            \) -print0 \
              | while IFS= read -r -d "" artifact_file
              do
                case "$artifact_file" in
                  *.a|*.o)
                    strip -g "$artifact_file"
                    ;;
                esac

                while IFS= read -r build_only_ref
                do
                  remove-references-to -t "$build_only_ref" "$artifact_file"
                done < <(
                  strings "$artifact_file" \
                    | rg -o '/nix/store/[[:alnum:]]{32}-[^/[:space:]]+' \
                    | sort -u \
                    | rg '^/nix/store/[[:alnum:]]{32}-(vendor-cargo-deps|vendor-registry|cargo-package-[^[:space:]]*|gcc-[^[:space:]]*|glibc-[^[:space:]]*)$'
                )
              done
          fi
        '';

        cargoArtifactSanitizers = {
          nativeBuildInputs = [
            pkgs.binutils
            pkgs.removeReferencesTo
            pkgs.ripgrep
          ];
          postBuild = sanitizeCargoArtifacts;
          postCheck = sanitizeCargoArtifacts;
        };

        cargoArtifacts = craneLib.buildDepsOnly ({
          pname = "${runnerPname}-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          doCheck = false;
          cargoExtraArgs = "-p runner --locked";
        } // cargoArtifactSanitizers);

        cargoArtifactsClosureCheck = pkgs.runCommand "${runnerPname}-cargo-artifacts-closure" {
          nativeBuildInputs = [
            pkgs.binutils
            pkgs.findutils
            pkgs.gnugrep
            pkgs.gnutar
            pkgs.zstd
          ];
        } ''
          artifact_tar="${cargoArtifacts}/target.tar.zst"
          cp "$artifact_tar" "$out"
          forbidden_ref_pattern='/nix/store/[[:alnum:]]{32}-(gcc-[^[:space:]]*|glibc-[^[:space:]]*-dev|vendor-cargo-deps|vendor-registry|cargo-package-[^[:space:]]*)'

          while IFS= read -r artifact_path
          do
            if tar --zstd -xOf "$artifact_tar" "$artifact_path" \
              | strings \
              | rg -o '/nix/store/[[:alnum:]]{32}-[^/[:space:]]+' \
              | rg -v '^/nix/store/e{32}-' \
              | grep -Eq "$forbidden_ref_pattern"; then
              echo "cargo-artifacts must not retain build-only store references via $artifact_path" >&2
              exit 1
            fi
          done < <(
            tar --zstd -tf "$artifact_tar" | grep -E '^./release/(build/.*/out/.*\.(a|o)|deps/.*\.(rlib|rmeta))$'
          )
        '';

        cargoArtifactsRuntimeBoundaryCheck = pkgs.runCommand "${runnerPname}-cargo-artifacts-runtime-boundary" {
          nativeBuildInputs = [
            pkgs.gnugrep
            pkgs.gnutar
            pkgs.zstd
          ];
        } ''
          artifact_tar="${cargoArtifacts}/target.tar.zst"
          cp "$artifact_tar" "$out"

          for forbidden_crate in reqwest tempfile
          do
            if tar --zstd -tf "$artifact_tar" | grep -Eq "/(lib)?''${forbidden_crate}[-_.]"; then
              echo "runtime cargo-artifacts must not retain dev-only crate ''${forbidden_crate}" >&2
              exit 1
            fi
          done
        '';

        runnerClippyCargoArtifacts = craneLib.buildDepsOnly ({
          pname = "${runnerPname}-clippy-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          doCheck = false;
          cargoExtraArgs = "--workspace --locked";
          cargoCheckExtraArgs = "--all-targets";
        } // cargoArtifactSanitizers);

        runner = craneLib.buildPackage {
          inherit cargoArtifacts;
          pname = runnerPname;
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoExtraArgs = "-p runner";
          doCheck = false;
        };

        runnerRuntimeCargoArtifacts = craneLibMusl.buildDepsOnly ({
          pname = "${runnerPname}-runtime-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          doCheck = false;
          cargoExtraArgs = "-p runner --locked";
          CARGO_BUILD_TARGET = runnerMuslTarget;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        } // cargoArtifactSanitizers);

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

        runnerTestCargoArtifacts = craneLib.buildDepsOnly ({
          pname = "${runnerPname}-test-deps";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoExtraArgs = "--workspace --locked";
          cargoCheckExtraArgs = "--all-targets";
          cargoTestExtraArgs = "--no-run";
        } // cargoArtifactSanitizers);

        runnerClippy = craneLib.cargoClippy {
          cargoArtifacts = runnerClippyCargoArtifacts;
          pname = "${runnerPname}-clippy";
          version = runnerVersion;
          src = cargoSrc;
          strictDeps = true;
          cargoClippyExtraArgs = "--workspace --all-targets -- -D warnings";
        };

        runnerTest = craneLib.cargoTest {
          cargoArtifacts = runnerTestCargoArtifacts;
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
          vendorHash = "sha256-7yHLVPjLmZxUbe9MxCzK3jqIPWEV27XKFQl/0yDgt4o=";
          subPackages = [ "." ];
          doCheck = false;
        };

        moltRuntime = pkgs.pkgsStatic.buildGoModule {
          pname = "molt-runtime";
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

        ciBuildBundleEntries =
          [
            {
              name = "runner-runtime-deps";
              path = runnerRuntimeCargoArtifacts;
            }
            {
              name = "runner-runtime";
              path = runnerRuntime;
            }
            {
              name = "verify-runtime";
              path = moltRuntime;
            }
          ]
          ++ pkgs.lib.optionals (system == "x86_64-linux") [
            {
              name = "runner-clippy-deps";
              path = runnerClippyCargoArtifacts;
            }
            {
              name = "runner-test-deps";
              path = runnerTestCargoArtifacts;
            }
          ];

        ciBuildBundle = pkgs.linkFarm "ci-build-bundle-${system}" ciBuildBundleEntries;

        checkApp =
          mkNixBuildApp
            "check"
            ".#checks.${system}.runner-clippy .#checks.${system}.cargo-artifacts-closure .#checks.${system}.cargo-artifacts-runtime-boundary";
        lintApp =
          mkNixBuildApp
            "lint"
            ".#checks.${system}.runner-clippy .#checks.${system}.cargo-artifacts-closure .#checks.${system}.cargo-artifacts-runtime-boundary";
        testApp = mkNixBuildApp "test" ".#checks.${system}.runner-test .#checks.${system}.verify-service-test";
        fmtApp = mkNixBuildApp "fmt" ".#checks.${system}.fmt-check";
        longTestApp = mkNixBuildApp "test-long" ".#packages.${system}.runner-long-test";
      in
      {
        packages = {
          default = runner;
          runner = runner;
          "cargo-artifacts" = cargoArtifacts;
          "ci-build-bundle" = ciBuildBundle;
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
          "cargo-artifacts-closure" = cargoArtifactsClosureCheck;
          "cargo-artifacts-runtime-boundary" = cargoArtifactsRuntimeBoundaryCheck;
          fmt-check = fmtCheck;
          runner-clippy = runnerClippy;
          runner-test = runnerTest;
          verify-service-test = verifyServiceTest;
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
      github.ciWorkflowCatalog = githubCiWorkflowCatalog;
    };
}
