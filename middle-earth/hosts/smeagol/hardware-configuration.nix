{
  mkFlakeModule,
  ...
}:
mkFlakeModule.withNixosModule { } (
  {
    config,
    lib,
    modulesPath,
    ...
  }:
  {
    imports = [
      (modulesPath + "/installer/scan/not-detected.nix")
    ];

    boot.initrd.availableKernelModules = [
      "xhci_pci"
      "ahci"
      "usbhid"
      "usb_storage"
      "sd_mod"
    ];
    boot.kernelModules = [
      "kvm-intel"
      "wl"
    ];
    boot.extraModulePackages = [
      config.boot.kernelPackages.broadcom_sta
    ];
    boot.blacklistedKernelModules = [
      "b43"
      "b43legacy"
      "bcma"
      "brcmfmac"
      "brcmsmac"
      "ssb"
    ];

    nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
    hardware.cpu.intel.updateMicrocode = lib.mkDefault config.hardware.enableRedistributableFirmware;
    hardware.enableAllFirmware = true;
    hardware.graphics.enable = true;
    hardware.bluetooth.enable = true;
    hardware.facetimehd.enable = true;

    services.mbpfan.enable = true;
  }
)
