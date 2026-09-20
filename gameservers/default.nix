{
  selfModule,
  terranix,
  nixpkgs,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};

  tofuWithPlugins = pkgs.opentofu.withPlugins (p: [
    p.lxc_incus
  ]);

  terraformConfiguration = terranix.lib.terranixConfiguration {
    inherit system;
    modules = [ selfModule.config ];
  };

  saveDirs = map (name: "/data/depot/games/saves/${name}") (builtins.attrNames selfModule.instances);

  makeRunner =
    name: op:
    pkgs.writeShellApplication {
      inherit name;
      runtimeInputs = [ tofuWithPlugins ];
      text = ''
        STATE_DIR="''${TOFU_STATE_DIR:-''${XDG_STATE_HOME:-$HOME/.local/state}/middle-earth/gameservers}"
        mkdir -p "$STATE_DIR" ${builtins.concatStringsSep " " saveDirs}
        ln -sf "${terraformConfiguration}" "$STATE_DIR/config.tf.json"
        cd "$STATE_DIR"
        tofu init -upgrade
        tofu ${op} "$@"
      '';
    };
in
{
  inherit terraformConfiguration;
  plan = makeRunner "plan" "plan";
  apply = makeRunner "apply" "apply";
  destroy = makeRunner "destroy" "destroy";
  buildPalworldImage = selfModule.images.palworld.buildApp;
}
