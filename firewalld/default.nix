{
  mkFlakeModule,
  ...
}:
mkFlakeModule { public = [ ./firewalld-policies.nix ]; } { }
