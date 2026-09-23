{
  createFlakeModule,
  ...
}:
createFlakeModule {
  win11-gollum = {
    image = "win11/26300.9457.pro.en-us/looking-glass/v2";
    uuid = "e5a7d620-8931-4bf6-98ec-7e44a30e8c45";
    mac = "52:54:00:11:00:01";
    kvmfrDev = "/dev/kvmfr0";
    vfFunction = "0x1";
    memory = 16777216; # KiB (16 GiB)
    cpus = 16;
  };

  win11-beruthiel = {
    image = "win11/26300.9457.pro.ja-jp/looking-glass/v2";
    uuid = "b2c81f24-4b38-4c7f-a0c7-cae44c33b063";
    mac = "52:54:00:11:00:02";
    kvmfrDev = "/dev/kvmfr1";
    vfFunction = "0x2";
    memory = 16777216; # KiB (16 GiB)
    cpus = 16;
  };
}
