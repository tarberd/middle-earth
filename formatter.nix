{
  mkFlakeModule,
  nixpkgs,
  ...
}:
mkFlakeModule { } {
  x86_64-linux = nixpkgs.legacyPackages.x86_64-linux.nixfmt-tree;
}
