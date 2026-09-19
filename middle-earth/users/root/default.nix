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
      artifacts.store.user-root = {
        prompts.password.description = "Password for user root";
        generator = pkgs.writeShellScript "gen-root-hash" ''
          ${pkgs.mkpasswd}/bin/mkpasswd -m sha-512 -s < "$prompts/password" > "$out/hashed_password"
        '';
        files.hashed_password = {
          owner = "root";
          group = "root";
          mode = "0400";
        };
      };

      users.users.${username} = {
        shell = pkgs.zsh;
        hashedPasswordFile = config.artifacts.store.user-root.files.hashed_password.path;
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
