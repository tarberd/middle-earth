{
  createFlakeModule,
  declareNixosModule,
  disko,
  ...
}:
createFlakeModule (
  declareNixosModule (
    { ... }:
    {
    imports = [ disko.nixosModules.disko ];

    disko.devices = {
      disk = {
        main = {
          type = "disk";
          device = "/dev/disk/by-id/nvme-WDS100T1X0E-00AFY0_21164S800370";
          content = {
            type = "gpt";
            partitions = {
              ESP = {
                priority = 1;
                name = "ESP";
                start = "1M";
                end = "512M";
                type = "EF00";
                content = {
                  type = "filesystem";
                  extraArgs = [ "-n" "BOOT" ];
                  format = "vfat";
                  mountpoint = "/boot";
                  mountOptions = [ "umask=0077" ];
                };
              };
              root = {
                size = "100%";
                name = "root";
                content = {
                  type = "btrfs";
                  extraArgs = [
                    "-f"
                    "-L nixos"
                    "-O block-group-tree"
                  ];
                  mountpoint = "/btrfs-root-partition";
                  subvolumes = {
                    "@" = {
                      mountpoint = "/";
                      mountOptions = [ "compress=zstd" "discard=async" ];
                    };
                    "@home" = {
                      mountpoint = "/home";
                      mountOptions = [ "compress=zstd" "discard=async" ];
                    };
                    "@nix" = {
                      mountpoint = "/nix";
                      mountOptions = [ "compress=zstd" "noatime" ];
                    };
                    "@snapshots" = {
                      mountpoint = "/snapshots";
                      mountOptions = [ "compress=zstd" "discard=async" ];
                    };
                  };
                };
              };
            };
          };
        };
      };
    };

    zramSwap.enable = true;

    fileSystems."/data" = {
      device = "/dev/disk/by-label/stanley-data";
      fsType = "btrfs";
      options = [ "subvol=@" "compress=zstd" "discard=async" "nofail" ];
    };
  }
  )
)
