{
  createFlakeModule,
  mod,
  self,
  flake,
  ...
}:
mod "onehost"
createFlakeModule {
  x86_64-linux = {
    gameservers-tf = flake.gameservers.terraformConfiguration;
    default = flake.gameservers.terraformConfiguration;
    onehost = self.onehost;
    onehost-manifest = flake.middle-earth.hosts.gandalf.virtualization.onehost.manifest;
  };
}
