{ middle-earth, home-manager, ... }:
{ pkgs, lib, config, ... }:
let
  username = "root";
in
{
  imports = [
    home-manager.nixosModules.home-manager
    middle-earth.users
  ];

  config = {
    users.users.${username} = {
      shell = pkgs.zsh;
    };

    home-manager.users.${username}.home = {
      stateVersion = "26.05";

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
