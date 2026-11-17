{
  mkFlakeModule,
  ...
}:
mkFlakeModule {
  public = [
    ./gandalf
    ./sauron
    ./smeagol
  ];
} { }
