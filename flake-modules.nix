{
  createFlakeModule,
  pub,
  mod,
  ...
}:
# Internal project namespaces
mod "middle-earth"
mod "gameservers"
mod "firewalld"
mod "dotman2nix"

# Public flake outputs
pub mod "apps"
pub mod "packages"
pub mod "nixosConfigurations"

createFlakeModule {}
