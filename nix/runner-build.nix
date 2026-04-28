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

    nativeTarget = targetForSystem.${system};

    rustToolchainFor =
      targets: p:
      p.rust-bin.stable.latest.default.override {
        inherit targets;
      };

    nativeRustToolchain = rustToolchainFor [ nativeTarget ];

    craneLib = (crane.mkLib pkgs).overrideToolchain nativeRustToolchain;

    pkgsForTarget =
      target:
      import nixpkgs {
        localSystem = system;
        crossSystem = {
          config = target;
        };
        overlays = [ (import rust-overlay) ];
      };

    craneLibForTarget =
      target: (crane.mkLib (pkgsForTarget target)).overrideToolchain (rustToolchainFor [ target ]);

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

    runnerCommonArgsFor =
      target:
      (envForTarget target)
      // {
        inherit src;
        strictDeps = true;
        pname = "runner";
        version = "0.1.0";
        cargoExtraArgs = "-p runner";
        doCheck = false;
      };

    buildRunnerFor =
      target: cargoArtifacts:
      let
        targetCraneLib = craneLibForTarget target;
        commonArgs = runnerCommonArgsFor target;
      in
      targetCraneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;
        }
      );

    runnerCommonArgs = runnerCommonArgsFor nativeTarget;

    nativeCraneLib = craneLibForTarget nativeTarget;

    runnerCargoArtifacts = nativeCraneLib.buildDepsOnly runnerCommonArgs;

    runner = buildRunnerFor nativeTarget runnerCargoArtifacts;

    runnerTest = import ./runner-test.nix {
      inherit
        pkgs
        flake-utils
        ;
      commonArgs = runnerCommonArgs;
      cargoArtifacts = runnerCargoArtifacts;
      craneLib = nativeCraneLib;
    };

    cargoArtifacts = nativeCraneLib.buildDepsOnly runnerCommonArgs;

    cargoCheck = nativeCraneLib.mkCargoDerivation {
      inherit src cargoArtifacts;
      strictDeps = true;
      pname = "runner";
      version = "0.1.0";
      pnameSuffix = "-check";
      buildPhaseCargoCommand = "cargoWithProfile check --locked -p runner --all-targets";
      CARGO_BUILD_TARGET = nativeTarget;
      CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
    };

    cargoClippy = nativeCraneLib.cargoClippy {
      inherit src cargoArtifacts;
      strictDeps = true;
      pname = "runner";
      version = "0.1.0";
      cargoExtraArgs = "-p runner";
      cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      CARGO_BUILD_TARGET = nativeTarget;
      CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
    };

    cargoFmt = craneLib.cargoFmt {
      inherit src;
      pname = "runner";
      version = "0.1.0";
    };

    successfulApp = name: pkgs.writeCBin name "int main(void) { return 0; }";

    checkArtifacts = pkgs.linkFarm "check-artifacts" [
      {
        name = "runner";
        path = runner;
      }
      {
        name = "cargoCheck";
        path = cargoCheck;
      }
      {
        name = "cargoClippy";
        path = cargoClippy;
      }
      {
        name = "cargoFmt";
        path = cargoFmt;
      }
    ];

    check = pkgs.symlinkJoin {
      name = "check";
      paths = [
        (successfulApp "check")
        checkArtifacts
      ];
    };

    lintArtifacts = pkgs.linkFarm "lint-artifacts" [
      {
        name = "cargoClippy";
        path = cargoClippy;
      }
    ];

    lint = pkgs.symlinkJoin {
      name = "lint";
      paths = [
        (successfulApp "lint")
        lintArtifacts
      ];
    };

    fmtArtifacts = pkgs.linkFarm "fmt-artifacts" [
      {
        name = "cargoFmt";
        path = cargoFmt;
      }
    ];

    fmt = pkgs.symlinkJoin {
      name = "fmt";
      paths = [
        (successfulApp "fmt")
        fmtArtifacts
      ];
    };

  in
  {
    packages = {
      inherit
        runner
        check
        lint
        fmt
        cargoCheck
        cargoClippy
        cargoFmt
        ;
      default = runner;
      runner-x86_64-linux =
        let
          target = targetForSystem.x86_64-linux;
        in
        buildRunnerFor target ((craneLibForTarget target).buildDepsOnly (runnerCommonArgsFor target));
      runner-aarch64-linux =
        let
          target = targetForSystem.aarch64-linux;
        in
        buildRunnerFor target ((craneLibForTarget target).buildDepsOnly (runnerCommonArgsFor target));
    }
    // runnerTest.packages;

    checks = {
      inherit
        runner
        cargoCheck
        cargoClippy
        cargoFmt
        ;
    }
    // runnerTest.checks;

    formatter = pkgs.nixfmt;

    apps = {
      default = flake-utils.lib.mkApp {
        drv = runner;
      };

      check = flake-utils.lib.mkApp {
        drv = check;
      };

      lint = flake-utils.lib.mkApp {
        drv = lint;
      };

      fmt = flake-utils.lib.mkApp {
        drv = fmt;
      };
    }
    // runnerTest.apps;

    devShells.default = craneLib.devShell {
      checks = self.checks.${system};
    };
  }
)
