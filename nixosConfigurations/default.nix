{
  createFlakeModule,
  flake,
  nixpkgs,
  nixos-artifacts,
  nixos-artifacts-agenix,
  ...
}:
let
  inherit (nixpkgs) lib;
in
createFlakeModule (builtins.mapAttrs (
  hostName: host:
    lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        nixos-artifacts.nixosModules.default
        nixos-artifacts-agenix.nixosModules.default
        {
          home-manager.useGlobalPkgs = true;
          home-manager.useUserPackages = true;
        }
        {
          specialisation.linked.configuration = {
            middle-earth.users.linkConfigs = lib.mkForce true;
          };
        }
        flake.middle-earth.users.root
        flake.middle-earth.users.tarberd
        host.configuration
      ];
    }
) flake.middle-earth.hosts)
