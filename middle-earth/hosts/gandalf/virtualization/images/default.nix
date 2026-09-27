{
  mkFlakeModule,
  ...
}:
mkFlakeModule {
  public = [ ./windows.nix ];
  private = [ ./windows-versions.nix ];
} { }
