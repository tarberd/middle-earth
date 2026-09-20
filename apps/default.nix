{
  gameservers,
  nixos-artifacts-agenix,
  ...
}:
{
  x86_64-linux = {
    artifacts = {
      type = "app";
      program = "${nixos-artifacts-agenix.packages.x86_64-linux.default}/bin/artifacts";
    };
    plan = {
      type = "app";
      program = "${gameservers.plan}/bin/plan";
    };
    apply = {
      type = "app";
      program = "${gameservers.apply}/bin/apply";
    };
    destroy = {
      type = "app";
      program = "${gameservers.destroy}/bin/destroy";
    };
    build-palworld-image = {
      type = "app";
      program = "${gameservers.buildPalworldImage}/bin/build-palworld-image";
    };
    default = {
      type = "app";
      program = "${gameservers.apply}/bin/apply";
    };
  };
}
