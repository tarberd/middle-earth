{
  mkFlakeModule,
  ...
}:
mkFlakeModule {
  public = [
    ./hosts
    ./users
  ];
} { }
