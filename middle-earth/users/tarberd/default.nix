{
  middle-earth,
  home-manager,
  dotman2nix,
  declareNixosModule,
  ...
}:
declareNixosModule (
  {
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
      middle-earth.userRoles.${username} = [ "admin" "virtualization" "desktop" ];

      users.users.${username} = {
        isNormalUser = true;
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
)
