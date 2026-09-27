{
  mkFlakeModule,
  ...
}:
mkFlakeModule {
  public = [
    ./kvm.nix
    ./container.nix
    ./instances.nix
    ./images
    ./onehost.nix
  ];
} { }
