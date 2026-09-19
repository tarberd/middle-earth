{
  nixpkgs,
  middle-earth,
  nixos-artifacts,
  nixos-artifacts-agenix,
  ...
}:
let
  inherit (nixpkgs) lib;
in
builtins.mapAttrs (
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
        middle-earth.users.root
        middle-earth.users.tarberd
        host.configuration
      ];
    }
) middle-earth.hosts
