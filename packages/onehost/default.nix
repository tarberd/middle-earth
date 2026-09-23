{
  createFlakeModule,
  nixpkgs,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};
in
createFlakeModule (pkgs.rustPlatform.buildRustPackage {
  pname = "onehost";
  version = "0.1.0";
  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;
})
