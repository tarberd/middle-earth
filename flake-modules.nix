{
  mkFlakeModule,
  ...
}:
mkFlakeModule {
  private = [
    ./middle-earth
    ./gameservers
    ./firewalld
    ./dotman2nix
  ];
  public = [
    ./apps
    ./packages
    ./nixosConfigurations
    ./formatter.nix
  ];
} { }
