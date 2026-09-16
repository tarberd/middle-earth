{
  gameservers,
  ...
}:
{
  x86_64-linux = {
    gameservers-tf = gameservers.terraformConfiguration;
    default = gameservers.terraformConfiguration;
  };
}
