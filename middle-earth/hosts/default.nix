{
  mkFlakeModule,
  ...
}:
mkFlakeModule {
  public = [
    ./gandalf
    ./sauron
  ];
} { }
