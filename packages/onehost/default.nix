{
  createFlakeModule,
  nixpkgs,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};
  lib = pkgs.lib;
in
createFlakeModule (pkgs.rustPlatform.buildRustPackage {
  pname = "onehost";
  version = "0.1.0";
  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [ pkgs.makeWrapper ];
  nativeCheckInputs = [ pkgs.qemu-utils ];
  postInstall = ''
    wrapProgram $out/bin/onehost \
      --prefix PATH : ${lib.makeBinPath [ pkgs.libvirt pkgs.qemu_kvm pkgs.swtpm ]}
  '';
})
