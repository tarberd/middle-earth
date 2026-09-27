{
  mkFlakeModule,
  ...
}:
mkFlakeModule {
  public = [
    ./configuration.nix
    ./virtualization
  ];
  private = [
    ./hardware-configuration.nix
    ./storage.nix
    ./network.nix
    ./backup.nix
  ];
} { }
