{ firewalld, ... }:
{ ... }:
{
  imports = [
    firewalld.firewalld-policies
  ];

  boot.kernelParams = [ "net.ifnames=0" ];
  boot.kernel.sysctl = {
    "net.ipv4.conf.all.forwarding" = 1;
    "net.ipv6.conf.all.forwarding" = 1;
  };

  networking = {
    hostName = "gandalf";
    networkmanager.enable = true;

    firewall.enable = false;
    nftables.enable = true;
    nftables.flushRuleset = true;
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
        interfaces = [ "virbr0" ];
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
