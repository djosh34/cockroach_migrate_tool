{
  pkgs,
  craneLib,
  flake-utils,
  commonArgs,
  cargoArtifacts,
}:
let
  cargoNextest = craneLib.cargoNextest (
    commonArgs
    // {
      inherit cargoArtifacts;
      doCheck = true;
      partitions = 1;
      partitionType = "count";
      cargoNextestPartitionsExtraArgs = "--no-tests=fail";
    }
  );

  successfulApp = name: pkgs.writeCBin name "int main(void) { return 0; }";

  testArtifacts = pkgs.linkFarm "test-artifacts" [
    {
      name = "cargoNextest";
      path = cargoNextest;
    }
  ];

  test = pkgs.symlinkJoin {
    name = "test";
    paths = [
      (successfulApp "test")
      testArtifacts
    ];
  };
in
{
  packages = {
    inherit cargoNextest test;
  };

  checks = {
    inherit cargoNextest;
  };

  apps = {
    test = flake-utils.lib.mkApp {
      drv = test;
    };
  };
}
