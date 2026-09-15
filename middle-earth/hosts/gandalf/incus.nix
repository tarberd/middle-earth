{ ... }:
{
  pkgs,
  lib,
  ...
}:
let
  # Standalone game server profile generator
  # Configures MTU (1420), WireGuard gateway routing, and static IPv4/IPv6
  # Consumed by cloud-init images (e.g. images:archlinux/cloud, images:debian/12/cloud, images:ubuntu/24.04/cloud)
  makeGameServerProfile =
    {
      name,
      ip4,
      ip6,
    }:
    {
      inherit name;
      devices = {
        eth0 = {
          name = "eth0";
          nictype = "bridged";
          parent = "br-public-hosts";
          type = "nic";
        };
        root = {
          path = "/";
          pool = "servers";
          type = "disk";
        };
      };
      config = {
        "user.network-config" = ''
          version: 2
          ethernets:
            eth0:
              mtu: 1420
              addresses:
                - ${ip4}/24
                - ${ip6}/64
              routes:
                - to: default
                  via: 10.100.2.1
                - to: "::/0"
                  via: "2a0f:9400:738f:2::1"
              nameservers:
                addresses:
                  - 8.8.8.8
                  - 8.8.4.4
                  - 2001:4860:4860::8888
                  - 2001:4860:4860::8844
        '';
      };
    };

  # Declarative container IP assignments controlled from Nix
  gameServers = {
    "palworld-static" = {
      ip4 = "10.100.2.100";
      ip6 = "2a0f:9400:738f:2::100";
    };
    "factorio-static" = {
      ip4 = "10.100.2.101";
      ip6 = "2a0f:9400:738f:2::101";
    };
  };
in
{
  systemd.tmpfiles.rules = [
    "d /data/virtualization/incus/storage-pools/default 0700 root root -"
    "d /data/virtualization/incus/storage-pools/servers 0700 root root -"
  ];

  virtualisation.incus = {
    enable = true;

    ui.enable = true;

    preseed = {
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
      ] ++ (lib.mapAttrsToList (name: cfg: makeGameServerProfile { inherit name; inherit (cfg) ip4 ip6; }) gameServers);
    };
  };

  middle-earth.roles.virtualization = [ "incus-admin" ];

  environment.systemPackages = with pkgs; [
    incus
  ];
}
