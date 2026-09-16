{
  declareNixosModule,
  ...
}:
declareNixosModule (
  {
    pkgs,
    ...
  }:
  {
    systemd.tmpfiles.rules = [
      "d /data/virtualization/incus/storage-pools/default 0700 root root -"
      "d /data/virtualization/incus/storage-pools/servers 0700 root root -"
    ];

    virtualisation.incus = {
      enable = true;

      ui.enable = true;

      preseed = {
        config = {
          "core.https_address" = "127.0.0.1:8443";
        };

        networks = [
          {
            name = "incusbr0";
            type = "bridge";
            config = {
              "ipv4.address" = "10.101.0.1/24";
              "ipv4.nat" = "true";
              "ipv6.address" = "none";
            };
          }
        ];

        storage_pools = [
          {
            name = "default";
            driver = "btrfs";
            config = {
              source = "/data/virtualization/incus/storage-pools/default";
            };
          }
          {
            name = "servers";
            driver = "btrfs";
            config = {
              source = "/data/virtualization/incus/storage-pools/servers";
            };
          }
        ];

        profiles = [
          {
            name = "default";
            devices = {
              eth0 = {
                name = "eth0";
                network = "incusbr0";
                type = "nic";
              };
              root = {
                path = "/";
                pool = "default";
                type = "disk";
              };
            };
          }
        ];
      };
    };

    middle-earth.roles.virtualization = [ "incus-admin" ];

    environment.systemPackages = with pkgs; [
      incus
      opentofu
    ];
  }
)
