{
  createFlakeModule,
  pub,
  mod,
  ...
}:
pub mod "kvm"
pub mod "container"
pub mod "instances"
pub mod "images"

createFlakeModule {}
