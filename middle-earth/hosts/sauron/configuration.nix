{
  firewalld,
  superModule,
  declareNixosModule,
  ...
}:
declareNixosModule (
  {
    modulesPath,
    lib,
    pkgs,
    ...
  }:
  {
    imports = [
      (modulesPath + "/installer/scan/not-detected.nix")
      (modulesPath + "/profiles/qemu-guest.nix")
      firewalld.firewalld-policies
      superModule.disko-config
    ];

    system.stateVersion = "26.05";

    boot.kernelParams = [
      "net.ifnames=0"
    ];

    boot.kernel.sysctl = {
      "net.ipv6.conf.all.forwarding" = 1;
      "net.ipv4.conf.all.forwarding" = 1;
    };

    boot.loader.grub = {
      efiSupport = true;
      efiInstallAsRemovable = true;
    };

    networking = {
      hostName = "sauron";
      useDHCP = false;
      interfaces = {
        eth0 = {
          ipv4.addresses = [
            { address = "206.83.40.77"; prefixLength = 24; }
          ];
          ipv6.addresses = [
            { address = "2a0f:9400:fa0:44::1"; prefixLength = 44; }
          ];
        };
      };
      wg-quick.interfaces = {
        wg0 = {
          address = [
            "10.100.1.1/24"
            "2a0f:9400:738f:1::1/64"
          ];
          listenPort = 51820;
          privateKeyFile = "/root/wireguard-keys/private";
          peers = [
            { # stanley
              publicKey = "VNpR6K59HlEE9CRAiDxTkbFyZ0e5HCG8a+x7uyAdTmg=";
              allowedIPs = [
                "10.100.1.2/32"
                "2a0f:9400:738f:1::2/128"
                "10.100.2.0/24"
                "2a0f:9400:738f:2::/64"
              ];
            }
          ];
        };
      };
      defaultGateway = {
        address = "206.83.40.1";
        interface = "eth0";
      };
      defaultGateway6 = {
        address = "2a0f:9400:fa0::1";
        interface = "eth0";
      };
      nameservers = [
        "8.8.8.8"
        "8.8.4.4"
        "2001:4860:4860::8888"
        "2606:4700:4700::8844"
      ];
      firewall.enable = false;
      nftables.enable = true;
      nftables.flushRuleset = true;
    };

    services.openssh.enable = true;

    services.firewalld = {
      enable = true;

      zones = {
        public = {
          interfaces = [ "eth0" ];
          services = [ "wireguard" "ssh" ];
          protocols = [ "icmp" "ipv6-icmp"];
          masquerade = true;
          forwardPorts = [
            {
              port = 8211;
              protocol = "udp";
              to-port = 8211;
              to-addr = "10.100.2.100";
            }
            {
              port = 34197;
              protocol = "udp";
              to-port = 34197;
              to-addr = "10.100.2.101";
            }
          ];
        };
        trusted = {
          interfaces = [ "wg0" ];
        };
      };

      services.palworld = {
        short = "Palworld Game Server";
        ports = [ { port = 8211; protocol = "udp"; } ];
      };

      policies = {
        vpn-inbound = {
          target = "CONTINUE";
          ingressZones = [ "public" ];
          egressZones = [ "trusted" ];
          protocols = [ "icmp" "ipv6-icmp" ];
          services = [ "palworld" "factorio" ];
        };
        vpn-outbound = {
          target = "ACCEPT";
          ingressZones = [ "trusted" ];
          egressZones = [ "public" ];
        };
      };
    };

    nix.settings.experimental-features = [ "nix-command" "flakes" ];

    middle-earth.roles = {
      admin = [ "wheel" ];
    };

    users.users.root = {
      hashedPassword = "$6$NvAm.r/Vdj43Y4gA$snMm90T2nBGPKRJjeCnAlHpcw/CtngbaIyE1Pc.NCd5JwhZbaudHGhtShPS4dI.ZRiWo30zKjR06rLQFdbhro.";
      openssh.authorizedKeys.keys = [
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB6a46GEO27tNA42ehDQkZClA4oNWypBDiOyc86OkNWO bernardo.mferrari@gmail.com"
      ];
    };

    programs.zsh.enable = true;
    environment.systemPackages = map lib.lowPrio [
      pkgs.neovim
      pkgs.curl
      pkgs.gitMinimal
      pkgs.wireguard-tools
    ];
  }
)
