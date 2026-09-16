{
  declareNixosModule,
  ...
}:
declareNixosModule (
  {
    lib,
    config,
    ...
  }:
  {
    options = {
      middle-earth.users.linkConfigs = lib.mkEnableOption "declarative dotfile linking via Home Manager";

      # 1. Hosts declare which system groups each role receives on this machine
      middle-earth.roles = lib.mkOption {
        type = lib.types.attrsOf (lib.types.listOf lib.types.str);
        default = {};
        description = "Mapping of role names to extraGroups granted by the host.";
      };

      # 2. Users declare which roles they have
      middle-earth.userRoles = lib.mkOption {
        type = lib.types.attrsOf (lib.types.listOf lib.types.str);
        default = {};
        description = "Mapping of username to assigned roles (e.g. admin, virtualization, desktop).";
      };
    };

    # 3. Automatically map each user's declared roles to the host's granted groups
    config.users.users = lib.mapAttrs (_username: roles: {
      extraGroups = lib.concatMap (role: config.middle-earth.roles.${role} or []) roles;
    }) config.middle-earth.userRoles;
  }
)
