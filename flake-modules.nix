{
  createFlakeModule,
  pubMod,
  mod,
  ...
}:
# Internal project namespaces
mod "middle-earth"
mod "gameservers"
mod "firewalld"
mod "dotman2nix"

# Public flake outputs
pubMod "apps"
pubMod "packages"
pubMod "nixosConfigurations"

createFlakeModule {}
