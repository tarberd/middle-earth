{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.firewalld;
  format = pkgs.formats.xml { };
  lib' = import ./lib.nix { inherit lib; };
  inherit (lib')
    filterNullAttrs
    mkPortOption
    mkXmlAttr
    portProtocolOptions
    protocolOption
    toXmlAttrs
    ;
in
{
  # 1. Extend the system schema with our new 'policies' option
  options.services.firewalld.policies = lib.mkOption {
    description = "Declarative firewalld routing policies.";
    default = {};
    type = lib.types.attrsOf (lib.types.submodule {
      options = {
        target = lib.mkOption {
          type = lib.types.enum [ "CONTINUE" "ACCEPT" "REJECT" "DROP" ];
          default = "CONTINUE";
          description = "The default action for packets matching this policy.";
        };
        ingressZones = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [];
          description = "Zones where traffic originates.";
        };
        egressZones = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [];
          description = "Zones where traffic exits.";
        };
        protocols = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [];
          description = "Allowed protocols (e.g., icmp, ipv6-icmp).";
        };
        services = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [];
          description = "Allowed services (e.g., ssh, http).";
        };
      };
    });
  };

  # 2. Generate the XML files only if firewalld is enabled and policies exist
  config = lib.mkIf (cfg.enable && cfg.policies != {}) {
    environment.etc = lib.mapAttrs' (
      name: value:
      lib.nameValuePair "firewalld/policies/${name}.xml" {
        source = format.generate "firewalld-policy-${name}.xml" {
          policy =
            let
              mkXmlAttrList = name: map(mkXmlAttr name);
            in
            filterNullAttrs (
              lib.mergeAttrsList [
                (toXmlAttrs { inherit (value) target; })
                {
                  ingress-zone = mkXmlAttrList "name" value.ingressZones;
                  egress-zone = mkXmlAttrList "name" value.egressZones;
                  protocol = mkXmlAttrList "value" value.protocols;
                  service = mkXmlAttrList "name" value.services;
                }
              ]
            );
        };
      }
    ) cfg.policies;
  };
}
