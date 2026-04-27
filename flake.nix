{
  description = "Crane-backed local workflows for cockroach_migrate_tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      crane,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;
        cargoSrc = craneLib.cleanCargoSource ./.;
        repoSrc = pkgs.lib.cleanSource ./.;
        runnerManifest = builtins.fromTOML (builtins.readFile ./crates/runner/Cargo.toml);
        runnerPname = runnerManifest.package.name;
        runnerVersion = runnerManifest.package.version;
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

        verifyService = pkgs.writeShellApplication {
          name = "verify-service";
          text = ''
            exec ${molt}/bin/molt verify-service "$@"
          '';
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
        testApp =
          mkNixBuildApp "test" ".#checks.${system}.runner-test .#checks.${system}.verify-service-test";
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
          "runner-long-test" = runnerLongTest;
          "test-long" = longTestApp;
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
    );
}
