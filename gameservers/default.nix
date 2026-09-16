{
  selfModule,
  terranix,
  nixpkgs,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};

  terraformConfiguration = terranix.lib.terranixConfiguration {
    inherit system;
    modules = [ selfModule.config ];
  };

  makeRunner =
    name: op:
    pkgs.writeShellApplication {
      inherit name;
      runtimeInputs = [ pkgs.opentofu ];
      text = ''
        STATE_DIR="''${TOFU_STATE_DIR:-$PWD/gameservers}"
        mkdir -p "$STATE_DIR"
        ln -sf "${terraformConfiguration}" "$STATE_DIR/config.tf.json"
        cd "$STATE_DIR"
        tofu init
        tofu ${op} "$@"
      '';
    };
in
{
  inherit terraformConfiguration;
  plan = makeRunner "plan" "plan";
  apply = makeRunner "apply" "apply";
  destroy = makeRunner "destroy" "destroy";
}
