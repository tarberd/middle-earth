{
  createFlakeModule,
  pubMod,
  ...
}:
pubMod "kvm"
pubMod "container"
pubMod "instances"
pubMod "images"
createFlakeModule {}
