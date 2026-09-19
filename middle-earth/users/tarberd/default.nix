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

    hasDesktopRole =
      (lib.elem "desktop" (config.middle-earth.userRoles.${username} or [ ]))
      && (config.middle-earth.roles ? desktop);
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
        openssh.authorizedKeys.keys = [
          "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB6a46GEO27tNA42ehDQkZClA4oNWypBDiOyc86OkNWO bernardo.mferrari@gmail.com"
        ];
        hashedPasswordFile = lib.mkIf hasDesktopRole config.artifacts.store.user-tarberd.files.hashed_password.path;
      };

      artifacts.store = lib.mkIf hasDesktopRole {
        user-tarberd = {
          prompts.password.description = "Password for user tarberd";
          generator = pkgs.writeShellScript "gen-tarberd-hash" ''
            ${pkgs.mkpasswd}/bin/mkpasswd -m sha-512 -s < "$prompts/password" > "$out/hashed_password"
          '';
          files.hashed_password = {
            owner = "root";
            group = "root";
            mode = "0400";
          };
        };
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
