{
  nixpkgs,
  middle-earth,
  ...
}: (
  builtins.mapAttrs (
    _: host: (
      nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [ host.configuration ];
      }
    )
  ) (
    middle-earth.hosts
  )
)

