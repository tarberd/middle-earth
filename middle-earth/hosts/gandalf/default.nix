{
  createFlakeModule,
  mod,
  pubMod,
  ...
}:
mod "hardware-configuration"
mod "storage"
mod "network"
mod "backup"
pubMod "configuration"
pubMod "virtualization"
createFlakeModule {}
