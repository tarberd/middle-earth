{
  middle-earth,
  home-manager,
  dotman2nix,
  ...
}: {
  pkgs,
  lib,
  config,
  ...
}:
let
  username = "tarberd";
  home = "/home/${username}";
  dotmanProfilePath = ../../../dotfiles/sushi;
in
{
  imports = [
    home-manager.nixosModules.home-manager
    middle-earth.users
  ];

  config = {
    users.users.${username} = {
      isNormalUser = true;
      extraGroups = [ "wheel" "libvirtd" ];
      #extraGroups = middle-earth.users.sudoUserExtraGroups;
      shell = pkgs.zsh;
    };

    home-manager.users.${username}.home = {
      stateVersion = "26.05";

      username = username;
      homeDirectory = home;

      packages = with pkgs; [
        git
        neovim
        tmux
        ripgrep
        bat
        code2prompt
      ];

      file = lib.mkIf config.middle-earth.users.linkConfigs (
        dotman2nix.parseDotmanProfile dotmanProfilePath
      );
    };
  };
}
