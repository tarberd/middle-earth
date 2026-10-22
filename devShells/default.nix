{
  mkFlakeModule,
  nixpkgs,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};
in
mkFlakeModule { } {
  x86_64-linux = {
    onehost = pkgs.mkShell {
      packages = [
        pkgs.rustc
        pkgs.cargo
        pkgs.clippy
        pkgs.gcc
        pkgs.qemu-utils
      ];
    };
  };
}
