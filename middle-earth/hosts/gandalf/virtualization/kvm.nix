{
  nixvirt,
  declareNixosModule,
  middle-earth,
  ...
}:
declareNixosModule (
  {
    pkgs,
    lib,
    config,
    ...
  }:
  let
    intelGpuSriov = pkgs.writeShellApplication {
      name = "intel-gpu-sriov";
      runtimeInputs = [
        pkgs.coreutils
        pkgs.pciutils
        pkgs.gnugrep
      ];
      text = ''
        SYS_PATH="/sys/bus/pci/devices/0000:07:00.0/sriov_numvfs"
        CMD="''${1:-status}"

        case "$CMD" in
          status)
            CURRENT="$(cat "$SYS_PATH" 2>/dev/null || echo "0")"
            echo "Intel Arc Pro B70 SR-IOV Status:"
            echo "  sriov_numvfs: $CURRENT"
            if [ "$CURRENT" = "0" ]; then
              echo "  Mode: Host LLM Mode (100% VRAM allocated to host 'xe' driver)"
            else
              echo "  Mode: VM Accelerated Mode ($CURRENT VF(s) active)"
              lspci -nnk -d 8086: | grep -E "07:00" -A 2 || true
            fi
            ;;
          enable)
            CURRENT="$(cat "$SYS_PATH" 2>/dev/null || echo "0")"
            TARGET="''${2:-2}"
            if [ "$CURRENT" != "$TARGET" ]; then
              echo "Enabling $TARGET SR-IOV Virtual Function(s)..."
              if [ "$CURRENT" != "0" ]; then
                echo 0 > "$SYS_PATH"
                sleep 0.2
              fi
              echo "$TARGET" > "$SYS_PATH"
              sleep 0.5
            fi
            echo "SR-IOV active with $(cat "$SYS_PATH") VF(s)."
            ;;
          disable)
            CURRENT="$(cat "$SYS_PATH" 2>/dev/null || echo "0")"
            if [ "$CURRENT" != "0" ]; then
              echo "Disabling SR-IOV Virtual Functions to restore host VRAM..."
              echo 0 > "$SYS_PATH"
            fi
            echo "SR-IOV disabled (sriov_numvfs = 0)."
            ;;
          *)
            echo "Usage: intel-gpu-sriov [status|enable [num]|disable]"
            exit 1
            ;;
        esac
      '';
    };

    lookingGlassGollum = pkgs.writeShellScriptBin "looking-glass-gollum" ''
      exec ${pkgs.looking-glass-client}/bin/looking-glass-client -f /dev/kvmfr0 "$@"
    '';

    lookingGlassBeruthiel = pkgs.writeShellScriptBin "looking-glass-beruthiel" ''
      exec ${pkgs.looking-glass-client}/bin/looking-glass-client -f /dev/kvmfr1 "$@"
    '';

  in
  {
    imports = [ nixvirt.nixosModules.default ];

    boot.extraModulePackages = [ config.boot.kernelPackages.kvmfr ];
    boot.initrd.kernelModules = [ "kvmfr" ];
    boot.kernelParams = [ "kvmfr.static_size_mb=128,128" ];
    boot.extraModprobeConfig = ''
      options kvm_amd nested=1
      options kvmfr static_size_mb=128,128
    '';

    middle-earth.roles.virtualization = [ "libvirtd" ];

    environment.systemPackages = with pkgs; [
      dnsmasq
      virt-manager
      looking-glass-client
      intelGpuSriov
      lookingGlassGollum
      lookingGlassBeruthiel
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
                address = "10.101.1.1";
                netmask = "255.255.255.0";
                dhcp.range = { start = "10.101.1.2"; end = "10.101.1.254"; };
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
              target = { path = "/var/lib/libvirt/images"; };
            };
            active = true;
          }
          {
            definition = nixvirt.lib.pool.writeXML {
              name = "data-legacy";
              uuid = "b7bf2334-cbef-4198-ad75-1d44bb4d658a";
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
          group = "libvirtd"
          cgroup_device_acl = [
            "/dev/null", "/dev/full", "/dev/zero",
            "/dev/random", "/dev/urandom",
            "/dev/ptmx", "/dev/kvm", "/dev/kqemu",
            "/dev/rtc","/dev/hpet", "/dev/vfio/vfio",
            "/dev/kvmfr0",
            "/dev/kvmfr1"
          ]
        '';
      };
    };

    systemd.tmpfiles.rules = [
      "d /var/lib/libvirt/images 0775 root libvirtd -"
      "d /var/lib/libvirt/qemu/nvram 0775 root libvirtd -"
    ];

    systemd.services.libvirt-backup-vms = {
      description = "Nightly staging backup for Libvirt KVM Windows VMs";
      serviceConfig = {
        Type = "oneshot";
        User = "root";
        ExecStart = "${middle-earth.hosts.gandalf.virtualization.images.windows.backupApp}/bin/backup-windows-vm all";
      };
    };

    systemd.timers.libvirt-backup-vms = {
      description = "Nightly staging backup timer for Libvirt Windows VMs";
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = "01:00";
        Persistent = true;
      };
    };
  }
)
