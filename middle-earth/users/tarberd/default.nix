{ middle-earth, home-manager, ... }:
{ pkgs, lib, config, ... }:
let
  username = "tarberd";
  home = "/home/${username}";
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

      file = lib.mkIf config.middle-earth.users.linkConfigs {
        # Example: linking a directory from your dotfiles repo
        # ".config/waybar".source = inputs.my-dotfiles + "/waybar";
      };
    };
  };
}
