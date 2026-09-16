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
    username = "root";
    dotmanProfilePath = ../../../dotfiles/sushi;
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

        file = lib.mkIf config.middle-earth.users.linkConfigs (
          dotman2nix.parseDotmanProfile dotmanProfilePath
        );
      };
    };
  }
)
