{
  mkFlakeModule,
  ...
}:
mkFlakeModule {
  public = [ ./configuration.nix ];
  private = [ ./disko-config.nix ];
} { }
