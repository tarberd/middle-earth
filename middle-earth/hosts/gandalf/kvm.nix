{ nixvirt, ... }:
{ pkgs, lib, config, ... }:
{
  imports = [ nixvirt.nixosModules.default ];

  boot.extraModulePackages = [ config.boot.kernelPackages.kvmfr ];
  boot.initrd.kernelModules = [ "kvmfr" ];
  boot.kernelParams = [ "kvmfr.static_size_mb=128" ];
  boot.extraModprobeConfig = "options kvm_amd nested=1";

  environment.systemPackages = with pkgs; [
    dnsmasq
    virt-manager
    looking-glass-client
  ];

  services.udev.packages = lib.singleton (pkgs.writeTextFile
    {
      name = "kvmfr";
      text = ''
        SUBSYSTEM=="kvmfr", GROUP="kvm", MODE="0660", TAG+="uaccess"
      '';
      destination = "/etc/udev/rules.d/70-kvmfr.rules";
    }
  );

  virtualisation.libvirt = {
    enable = true;
    connections."qemu:///system" = {
      networks = [
        {
          definition = nixvirt.lib.network.writeXML {
            name = "default";
            uuid = "cda3b7dd-71fd-44e3-8093-340f47a88c83";
            bridge.name = "virbr0";
            forward = { mode = "nat"; };
            ip = {
              address = "10.100.100.1";
              netmask = "255.255.255.0";
              dhcp.range = { start = "10.100.100.2"; end = "10.100.100.224"; };
            };
          };
          active = true;
        }
        {
          definition = nixvirt.lib.network.writeXML {
            name = "sandbox";
            uuid = "703c2b85-da03-4e42-84d7-c57663ea14e7";
            bridge.name = "sandbox0";
            ip = {
              address = "10.67.67.1";
              netmask = "255.255.255.0";
              dhcp.range = { start = "10.67.67.2"; end = "10.67.67.254"; };
            };
          };
          active = true;
        }
      ];
      pools = [
        {
          definition = nixvirt.lib.pool.writeXML {
            name = "default";
            uuid = "8d1a7ca5-cd4a-4103-b488-c5f210552d33";
            type = "dir";
            target = { path = "/data/kvm/libvirt/images"; };
          };
          active = true;
        }
      ];
    };
  };

  virtualisation.libvirtd = {
    enable = true;
    qemu = {
      package = pkgs.qemu_kvm;
      runAsRoot = true;
      swtpm.enable = true;
      verbatimConfig = ''
        namespaces = []
        cgroup_device_acl = [
          "/dev/null", "/dev/full", "/dev/zero",
          "/dev/random", "/dev/urandom",
          "/dev/ptmx", "/dev/kvm", "/dev/kqemu",
          "/dev/rtc","/dev/hpet", "/dev/vfio/vfio",
          "/dev/kvmfr0"
        ]
      '';
    };
  };
}
