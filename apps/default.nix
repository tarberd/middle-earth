{
  createFlakeModule,
  flake,
  nixos-artifacts-agenix,
  ...
}:
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
      program = "${flake.middle-earth.hosts.gandalf.virtualization.images.windows.buildApp}/bin/build-windows-image";
    };
    provision-windows-vm = {
      type = "app";
      program = "${flake.middle-earth.hosts.gandalf.virtualization.images.windows.provisionApp}/bin/provision-windows-vm";
    };
    backup-windows-vm = {
      type = "app";
      program = "${flake.middle-earth.hosts.gandalf.virtualization.images.windows.backupApp}/bin/backup-windows-vm";
    };
    onehost = {
      type = "app";
      program = "${flake.packages.x86_64-linux.onehost}/bin/onehost";
    };
    default = {
      type = "app";
      program = "${flake.gameservers.apply}/bin/apply";
    };
  };
}
