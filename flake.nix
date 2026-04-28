{
  description = "Static runner build for cockroach_migrate_tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      targetForSystem = {
        x86_64-linux = "x86_64-unknown-linux-musl";
        aarch64-linux = "aarch64-unknown-linux-musl";
      };
    in
    flake-utils.lib.eachSystem systems (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        inherit (pkgs) lib;

        rustTargets = lib.attrValues targetForSystem;

        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p:
          p.rust-bin.stable.latest.default.override {
            targets = rustTargets;
          }
        );

        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./crates
          ];
        };

        envForTarget = target: {
          CARGO_BUILD_TARGET = target;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
        };

        buildRunnerFor =
          target:
          let
            targetEnv = envForTarget target;

            commonArgs = targetEnv // {
              inherit src;
              strictDeps = true;
              pname = "runner";
              version = "0.1.0";
              cargoExtraArgs = "-p runner";
              doCheck = false;
            };

            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          in
          craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );

        nativeTarget = targetForSystem.${system};

        runner = buildRunnerFor nativeTarget;

        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          strictDeps = true;
          pname = "runner";
          version = "0.1.0";
          cargoExtraArgs = "-p runner";
        };

        cargoCheck = craneLib.mkCargoDerivation {
          inherit src cargoArtifacts;
          strictDeps = true;
          pname = "runner";
          version = "0.1.0";
          pnameSuffix = "-check";
          buildPhaseCargoCommand = "cargoWithProfile check --locked -p runner --all-targets";
        };

        cargoClippy = craneLib.cargoClippy {
          inherit src cargoArtifacts;
          strictDeps = true;
          pname = "runner";
          version = "0.1.0";
          cargoExtraArgs = "-p runner";
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        };

        cargoFmt = craneLib.cargoFmt {
          inherit src;
          pname = "runner";
          version = "0.1.0";
        };
      in
      {
        packages = {
          inherit runner;
          default = runner;
          runner-x86_64-linux = buildRunnerFor targetForSystem.x86_64-linux;
          runner-aarch64-linux = buildRunnerFor targetForSystem.aarch64-linux;
        };

        checks = {
          inherit runner cargoCheck cargoClippy cargoFmt;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = runner;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
        };
      }
    );
}
