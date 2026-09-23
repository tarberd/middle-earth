{
  createFlakeModule,
  pubMod,
  ...
}:
pubMod "hosts"
pubMod "users"
createFlakeModule {}
