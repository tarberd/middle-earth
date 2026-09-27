{
  createFlakeModule,
  flake,
  nixos-artifacts-agenix,
  ...
}:
let
  onehostApps = flake.middle-earth.hosts.gandalf.virtualization.onehost.apps;
in
createFlakeModule {
  x86_64-linux = {
    artifacts = {
      type = "app";
      program = "${nixos-artifacts-agenix.packages.x86_64-linux.default}/bin/artifacts";
    };
    plan = {
      type = "app";
      program = "${flake.gameservers.plan}/bin/plan";
    };
    apply = {
      type = "app";
      program = "${flake.gameservers.apply}/bin/apply";
    };
    destroy = {
      type = "app";
      program = "${flake.gameservers.destroy}/bin/destroy";
    };
    build-palworld-image = {
      type = "app";
      program = "${flake.gameservers.buildPalworldImage}/bin/build-palworld-image";
    };
    build-windows-image = {
      type = "app";
      program = "${onehostApps.buildImage}/bin/onehost-build-image";
    };
    provision-windows-vm = {
      type = "app";
      program = "${onehostApps.apply}/bin/onehost-apply";
    };
    backup-windows-vm = {
      type = "app";
      program = "${onehostApps.backup}/bin/onehost-backup";
    };
    onehost = {
      type = "app";
      program = "${flake.packages.x86_64-linux.onehost}/bin/onehost";
    };
    onehost-plan = {
      type = "app";
      program = "${onehostApps.plan}/bin/onehost-plan";
    };
    onehost-apply = {
      type = "app";
      program = "${onehostApps.apply}/bin/onehost-apply";
    };
    onehost-destroy = {
      type = "app";
      program = "${onehostApps.destroy}/bin/onehost-destroy";
    };
    onehost-backup = {
      type = "app";
      program = "${onehostApps.backup}/bin/onehost-backup";
    };
    onehost-restore = {
      type = "app";
      program = "${onehostApps.restore}/bin/onehost-restore";
    };
    onehost-status = {
      type = "app";
      program = "${onehostApps.status}/bin/onehost-status";
    };
    onehost-build-image = {
      type = "app";
      program = "${onehostApps.buildImage}/bin/onehost-build-image";
    };
    default = {
      type = "app";
      program = "${flake.gameservers.apply}/bin/apply";
    };
  };
}
