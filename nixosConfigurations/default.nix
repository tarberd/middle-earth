{
  nixpkgs,
  middle-earth,
  ...
}: (
  builtins.mapAttrs (
    _: host: (
      nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          {
            home-manager.useGlobalPkgs = true;
            home-manager.useUserPackages = true;
          }
          middle-earth.users.root
          middle-earth.users.tarberd
          host.configuration
        ];
      }
    )
  ) (
    middle-earth.hosts
  )
)

