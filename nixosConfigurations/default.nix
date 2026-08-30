{
  nixpkgs,
  middle-earth,
  ...
}:
let
  inherit (nixpkgs) lib;
in
lib.listToAttrs (
  lib.concatMap (
    hostName: (
      let
        host = middle-earth.hosts.${hostName};
        makeSystem = linkConfigs: (
          lib.nixosSystem {
            system = "x86_64-linux";
            modules = [
              {
                home-manager.useGlobalPkgs = true;
                home-manager.useUserPackages = true;
              }
              {
                middle-earth.users.linkConfigs = lib.mkForce linkConfigs;
              }
              middle-earth.users.root
              middle-earth.users.tarberd
              host.configuration
            ];
          }
        );
      in [
        { name = hostName; value = makeSystem false; }
        { name = "${hostName}-linked"; value = makeSystem true; }
      ]
    )
  ) (
    builtins.attrNames middle-earth.hosts
  )
)
