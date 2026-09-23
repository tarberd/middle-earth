{
  createFlakeModule,
  super,
  ...
}:
createFlakeModule (
{
  lib,
  ...
}:
let
  instances = super.instances;
in
{
  terraform.required_providers.incus = {
    source = "lxc/incus";
    version = ">= 0.1.0";
  };

  provider.incus = { };

  resource.incus_instance = lib.mapAttrs (name: cfg: {
    inherit name;
    image = cfg.image or "images:archlinux/cloud";
    running = true;

    config =
      {
        "boot.autostart" = toString (cfg.autostart or true);
        "user.network-config" = ''
          version: 2
          ethernets:
            eth0:
              mtu: 1420
              addresses:
                - ${cfg.ip4}/24
                - ${cfg.ip6}/64
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
      }
      // (lib.optionalAttrs (cfg ? cpu) { "limits.cpu" = toString cfg.cpu; })
      // (lib.optionalAttrs (cfg ? memory) { "limits.memory" = cfg.memory; })
      // (cfg.extraConfig or { });

    device = [
      {
        name = "root";
        type = "disk";
        properties = {
          path = "/";
          pool = cfg.storagePool or "servers";
        };
      }
      {
        name = "eth0";
        type = "nic";
        properties = {
          nictype = "bridged";
          parent = cfg.parentBridge or "br-public-hosts";
        };
      }
      {
        name = "saves";
        type = "disk";
        properties = {
          source = "/data/depot/games/saves/${name}";
          path = cfg.savesPath or "/data";
          shift = "true";
        };
      }
    ] ++ (cfg.extraDevices or [ ]);
  }) instances;
})
