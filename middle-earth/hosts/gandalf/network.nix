{
  createFlakeModule,
  declareNixosModule,
  flake,
  ...
}:
createFlakeModule (
  declareNixosModule (
    {
      pkgs,
      config,
      ...
    }:
  {
    imports = [
      flake.firewalld.firewalld-policies
    ];

    environment.systemPackages = with pkgs; [
      wireguard-tools
    ];

    boot.kernelParams = [ "net.ifnames=0" ];
    boot.kernel.sysctl = {
      "net.ipv4.conf.all.forwarding" = 1;
      "net.ipv6.conf.all.forwarding" = 1;
    };

    networking = {
      hostName = "gandalf";
      networkmanager = {
        enable = true;
        unmanaged = [ "br-public-hosts" "wg0" "incusbr0" "interface-name:incus*" ];
      };

      firewall.enable = false;
      nftables.enable = true;
      nftables.flushRuleset = true;
    };

    systemd.network.enable = true;

    systemd.network.netdevs."10-br-public-hosts" = {
      netdevConfig = {
        Name = "br-public-hosts";
        Kind = "bridge";
        MTUBytes = 1420;
      };
    };

    systemd.network.networks."10-br-public-hosts" = {
      matchConfig.Name = "br-public-hosts";
      address = [
        "10.100.2.1/24"
        "2a0f:9400:738f:2::1/64"
      ];
      networkConfig = {
        IPv4Forwarding = "yes";
        IPv6Forwarding = "yes";
        ConfigureWithoutCarrier = true;
      };
      linkConfig = {
        RequiredForOnline = false;
        MTUBytes = 1420;
      };
    };

    artifacts.store.wireguard = {
      prompts.private_key.description = "Enter WireGuard private key for Gandalf (or press Enter to generate new)";
      generator = pkgs.writeShellScript "gen-wg-key" ''
        KEY="$(tr -d '[:space:]' < "$prompts/private_key")"
        if [ -n "$KEY" ]; then
          echo "$KEY" > "$out/private_key"
        else
          ${pkgs.wireguard-tools}/bin/wg genkey > "$out/private_key"
        fi
      '';
      files.private_key = {
        owner = "root";
        group = "systemd-network";
        mode = "0440";
      };
    };

    systemd.network.netdevs."10-wg0" = {
      netdevConfig = {
        Name = "wg0";
        Kind = "wireguard";
      };
      wireguardConfig = {
        PrivateKeyFile = config.artifacts.store.wireguard.files.private_key.path;
      };
      wireguardPeers = [
        {
          PublicKey = "avUBAFdY8UIrBI2+FewyfKUH9n5v/fevKBLpOTLmuAU=";
          Endpoint = "[2a0f:9400:fa0:44::1]:51820";
          AllowedIPs = [ "0.0.0.0/0" "::/0" ];
          PersistentKeepalive = 25;
          RouteTable = "100";
        }
      ];
    };

    systemd.network.networks."10-wg0" = {
      matchConfig.Name = "wg0";
      address = [
        "10.100.1.2/24"
        "2a0f:9400:738f:1::2/64"
      ];
      routingPolicyRules = [
        # Keep local subnet traffic in the main routing table
        { To = "10.100.2.0/24";        Table = 254; Priority = 990; }
        { To = "2a0f:9400:738f:2::/64"; Table = 254; Priority = 990; }
        # Route traffic originating from public IP block through table 100
        { From = "10.100.0.0/16";        Table = 100; Priority = 999; }
        { From = "2a0f:9400:738f::/48";  Table = 100; Priority = 999; }
      ];
    };

    services.firewalld = {
      enable = true;

      zones = {
        public = {
          interfaces = [ "eth0" "eth1" ];
          services = [ ];
          protocols = [ "icmp" "ipv6-icmp"];
          masquerade = true;
        };
        trusted = {
          interfaces = [ "virbr0" "wg0" "br-public-hosts" "incusbr0" ];
        };
      };

      policies = {
        public-to-trusted = {
          target = "CONTINUE";
          ingressZones = [ "public" ];
          egressZones = [ "trusted" ];
          protocols = [ "icmp" "ipv6-icmp" ];
        };
        trusted-to-public = {
          target = "ACCEPT";
          ingressZones = [ "trusted" ];
          egressZones = [ "public" ];
        };
      };
    };

  }
  )
)
