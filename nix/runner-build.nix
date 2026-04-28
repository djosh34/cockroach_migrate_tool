{
  self,
  nixpkgs,
  crane,
  flake-utils,
  rust-overlay,
  root,
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
      localSystem = system;
      overlays = [ (import rust-overlay) ];
    };

    inherit (pkgs) lib;

    rustToolchain =
      p:
      p.rust-bin.stable.latest.default.override {
        targets = lib.attrValues targetForSystem;
      };

    craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

    pkgsForTarget =
      target:
      import nixpkgs {
        localSystem = system;
        crossSystem = {
          config = target;
        };
        overlays = [ (import rust-overlay) ];
      };

    craneLibForTarget = target: (crane.mkLib (pkgsForTarget target)).overrideToolchain rustToolchain;

    src = lib.fileset.toSource {
      inherit root;
      fileset = lib.fileset.unions [
        (root + "/Cargo.toml")
        (root + "/Cargo.lock")
        (root + "/crates")
      ];
    };

    envForTarget = target: {
      CARGO_BUILD_TARGET = target;
      CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
    };

    buildRunnerFor =
      target:
      let
        targetCraneLib = craneLibForTarget target;
        targetEnv = envForTarget target;

        commonArgs = targetEnv // {
          inherit src;
          strictDeps = true;
          pname = "runner";
          version = "0.1.0";
          cargoExtraArgs = "-p runner";
          doCheck = false;
        };

        cargoArtifacts = targetCraneLib.buildDepsOnly commonArgs;
      in
      targetCraneLib.buildPackage (
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

    checkApp = pkgs.writeShellApplication {
      name = "check";
      runtimeInputs = [ pkgs.nix ];
      text = ''
        nix build --no-link \
          ${self}#checks.${system}.runner \
          ${self}#checks.${system}.cargoCheck \
          ${self}#checks.${system}.cargoClippy \
          ${self}#checks.${system}.cargoFmt
      '';
    };

    lintApp = pkgs.writeShellApplication {
      name = "lint";
      runtimeInputs = [ pkgs.nix ];
      text = ''
        nix build --no-link ${self}#checks.${system}.cargoClippy
      '';
    };

    fmtApp = pkgs.writeShellApplication {
      name = "fmt";
      runtimeInputs = [ pkgs.nix ];
      text = ''
        nix build --no-link ${self}#checks.${system}.cargoFmt
      '';
    };

  in
  {
    packages = {
      inherit
        runner
        cargoCheck
        cargoClippy
        cargoFmt
        ;
      default = runner;
      runner-x86_64-linux = buildRunnerFor targetForSystem.x86_64-linux;
      runner-aarch64-linux = buildRunnerFor targetForSystem.aarch64-linux;
    };

    checks = {
      inherit
        runner
        cargoCheck
        cargoClippy
        cargoFmt
        ;
    };

    formatter = pkgs.nixfmt;

    apps = {
      default = flake-utils.lib.mkApp {
        drv = runner;
      };

      check = flake-utils.lib.mkApp {
        drv = checkApp;
      };

      lint = flake-utils.lib.mkApp {
        drv = lintApp;
      };

      fmt = flake-utils.lib.mkApp {
        drv = fmtApp;
      };
    };

    devShells.default = craneLib.devShell {
      checks = self.checks.${system};
    };
  }
)
