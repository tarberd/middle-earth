{ ... }:
{
  palworld-arnh = {
    image = "images:archlinux/cloud";
    ip4 = "10.100.2.100";
    ip6 = "2a0f:9400:738f:2::100";
    cpu = 4;
    memory = "16GiB";
    autostart = true;
  };

  factorio-trutas = {
    image = "images:archlinux/cloud";
    ip4 = "10.100.2.101";
    ip6 = "2a0f:9400:738f:2::101";
    cpu = 2;
    memory = "4GiB";
    autostart = true;
  };
}
