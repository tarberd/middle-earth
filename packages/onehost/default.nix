{
  mkFlakeModule,
  nixpkgs,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};
  lib = pkgs.lib;
in
mkFlakeModule { } (
  pkgs.rustPlatform.buildRustPackage {
    pname = "onehost";
    version = "0.1.0";
    # Only what cargo needs, so that editing this file does not rebuild the package
    src = lib.fileset.toSource {
      root = ./.;
      fileset = lib.fileset.unions [
        ./Cargo.toml
        ./Cargo.lock
        ./src
        ./tests
      ];
    };
    cargoLock.lockFile = ./Cargo.lock;
    nativeBuildInputs = [ pkgs.makeWrapper ];
    nativeCheckInputs = [ pkgs.qemu-utils ];
    postInstall = ''
      wrapProgram $out/bin/onehost \
        --prefix PATH : ${
          lib.makeBinPath [
            pkgs.libvirt
            pkgs.qemu_kvm
            pkgs.swtpm
          ]
        }
    '';
  }
)
