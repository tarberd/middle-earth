{
  createFlakeModule,
  pub,
  mod,
  ...
}:
mod "hardware-configuration"
mod "storage"
mod "network"
mod "backup"

pub mod "configuration"
pub mod "virtualization"

createFlakeModule {}
