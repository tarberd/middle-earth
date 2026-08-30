{ ... }:
{ lib, ... }:
{
  options = {
    middle-earth.users.linkConfigs = lib.mkEnableOption "declarative dotfile linking via Home Manager";
  };
}
