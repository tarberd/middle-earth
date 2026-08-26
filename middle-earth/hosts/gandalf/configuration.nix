{ inputs, pkgs, lib, config, ... }:
{
  imports = [
    ../../../firewalld/firewalld-policies.nix
  ];
  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = true;
  boot.kernelPackages = pkgs.linuxPackages_latest;
  boot.extraModulePackages = [ config.boot.kernelPackages.kvmfr ];
  boot.initrd.kernelModules = [ "xe" "kvmfr" ];
  boot.kernelParams = [ "kvmfr.static_size_mb=128" "net.ifnames=0" ];
  boot.extraModprobeConfig = "options kvm_amd nested=1";
  boot.kernel.sysctl = {
    "net.ipv4.conf.all.forwarding" = 1;
    "net.ipv6.conf.all.forwarding" = 1;
  };

  nix.settings.experimental-features = [ "nix-command" "flakes" ];
  nix.settings.auto-optimise-store = true;
  nix.gc = {
    automatic = true;
    dates = "weekly";
    options = "--delete-older-than 14d";
  };

  networking = {
    hostName = "gandalf";
    networkmanager.enable = true;

    firewall.enable = false;
    nftables.enable = true;
    nftables.flushRuleset = true;
  };

  services.firewalld = {
    enable = true;

    zones = {
      public = {
        interfaces = [ "eth0" "eth1" ];
        services = [ ];
        protocols = [ "icmp" "ipv6-icmp"];
        masquerade = true;
      };
      trusted = {
        interfaces = [ "virbr0" ];
      };
    };

    policies = {
      public-to-trusted = {
        target = "CONTINUE";
        ingressZones = [ "public" ];
        egressZones = [ "trusted" ];
        protocols = [ "icmp" "ipv6-icmp" ];
      };
      trusted-to-public = {
        target = "ACCEPT";
        ingressZones = [ "trusted" ];
        egressZones = [ "public" ];
      };
    };
  };

  time.timeZone = "Europe/Amsterdam";
  i18n.defaultLocale = "en_US.UTF-8";

  console = {
    font = "Lat2-Terminus16";
    keyMap = "us";
  };

  zramSwap.enable = true;

  fileSystems."/data" = {
    device = "/dev/disk/by-label/stanley-data";
    fsType = "btrfs";
    options = [ "subvol=@" "compress=zstd" "discard=async" "nofail" ];
  };

  services.printing.enable = true;

  services.pipewire = {
    enable = true;
    alsa.enable = true;
    alsa.support32Bit = true;
    pulse.enable = true;
  };

  users.users.tarberd = {
    isNormalUser = true;
    extraGroups = [ "wheel" "libvirtd" ];
    packages = with pkgs; [
      looking-glass-client
    ];
    shell = pkgs.zsh;
  };

  programs.firefox.enable = true;
  programs.sway.enable = true;
  programs.zsh.enable = true;

  environment.systemPackages = with pkgs; [
    vim
    neovim
    git
    wget
    pciutils
    vulkan-tools
    openssh
    code2prompt
    wl-clipboard
    swaybg
    waybar
    pavucontrol
    python3
    pipx
    ripgrep
    nil
    nixd
    virt-manager
    tree
    polkit
    polkit_gnome
    dnsmasq
  ];

  nix.nixPath = [ "nixpkgs=${inputs.nixpkgs}" ];

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
          definition = inputs.nixvirt.lib.network.writeXML {
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
          definition = inputs.nixvirt.lib.network.writeXML {
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
          definition = inputs.nixvirt.lib.pool.writeXML {
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

  system.stateVersion = "26.05";
}
