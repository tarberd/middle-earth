{
  gameservers,
  ...
}:
{
  x86_64-linux = {
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
    default = {
      type = "app";
      program = "${gameservers.apply}/bin/apply";
    };
  };
}
