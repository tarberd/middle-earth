{
  createFlakeModule,
  flake,
  nixpkgs,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};
  lib = pkgs.lib;

  windows = flake.middle-earth.hosts.gandalf.virtualization.images.windows;
  instances = flake.middle-earth.hosts.gandalf.virtualization.instances;
  onehostPkg = flake.packages.x86_64-linux.onehost;

  lookingGlassHostZip = pkgs.fetchurl {
    url = "https://looking-glass.io/artifact/B7/host";
    name = "looking-glass-host.zip";
    sha256 = "06vv0jzf4frw25z788a59dbm0v409gfnp61nm5m1sps01id5lhf2";
  };

  mkOemdrv =
    {
      flavor,
      autounattend,
      sysprep,
      extraFiles,
    }:
    pkgs.runCommand "oemdrv-${flavor}.iso" {
      nativeBuildInputs = [ pkgs.cdrtools ];
    } ''
      mkdir -p root
      cp "${autounattend}" root/autounattend.xml
      cp "${sysprep}" root/sysprep.xml
      cp "${windows.provisionPs1}" root/provision.ps1
      cp "${windows.errorHandlerCmd}" root/ErrorHandler.cmd

      ${lib.concatStringsSep "\n" (
        lib.mapAttrsToList (name: path: ''
          cp "${path}" "root/${name}"
        '') extraFiles
      )}

      mkisofs -o "$out" -J -r -V "OEMDRV" root
    '';

  oemdrvFlavors = {
    looking-glass-en = mkOemdrv {
      flavor = "looking-glass-en";
      autounattend = windows.mkAutounattendXml {
        edition = "professional";
        language = "en-us";
        computerName = "WIN11-VM";
      };
      sysprep = windows.mkSysprepXml {
        language = "ja-jp";
        computerName = "WIN11-VM";
      };
      extraFiles = {
        "looking-glass-host.zip" = lookingGlassHostZip;
      };
    };
    looking-glass-jp = mkOemdrv {
      flavor = "looking-glass-jp";
      autounattend = windows.mkAutounattendXml {
        edition = "professional";
        language = "ja-jp";
        computerName = "WIN11-VM";
      };
      sysprep = windows.mkSysprepXml {
        language = "ja-jp";
        computerName = "WIN11-VM";
      };
      extraFiles = {
        "looking-glass-host.zip" = lookingGlassHostZip;
      };
    };
  };

  mkDomainVFIOTemplateXml =
    {
      name,
      uuid,
      mac,
      kvmfrDev,
      vfFunction,
      memory,
      cpus,
    }:
    pkgs.writeText "${name}-domain-template.xml" ''
      <domain type='kvm' xmlns:qemu='http://libvirt.org/schemas/domain/qemu/1.0' xmlns:onehost='https://middle-earth.internal/onehost'>
        <name>${name}</name>
        <uuid>${uuid}</uuid>
        <metadata>
          <libosinfo:libosinfo xmlns:libosinfo="http://libosinfo.org/xmlns/libvirt/domain/1.0">
            <libosinfo:os id="http://microsoft.com/win/11"/>
          </libosinfo:libosinfo>
        </metadata>
        <memory unit='KiB'>${toString memory}</memory>
        <currentMemory unit='KiB'>${toString memory}</currentMemory>
        <vcpu placement='static'>${toString cpus}</vcpu>
        <iothreads>1</iothreads>
        <cputune>
          <vcpupin vcpu='0' cpuset='8'/>
          <vcpupin vcpu='1' cpuset='24'/>
          <vcpupin vcpu='2' cpuset='9'/>
          <vcpupin vcpu='3' cpuset='25'/>
          <vcpupin vcpu='4' cpuset='10'/>
          <vcpupin vcpu='5' cpuset='26'/>
          <vcpupin vcpu='6' cpuset='11'/>
          <vcpupin vcpu='7' cpuset='27'/>
          <vcpupin vcpu='8' cpuset='12'/>
          <vcpupin vcpu='9' cpuset='28'/>
          <vcpupin vcpu='10' cpuset='13'/>
          <vcpupin vcpu='11' cpuset='29'/>
          <vcpupin vcpu='12' cpuset='14'/>
          <vcpupin vcpu='13' cpuset='30'/>
          <vcpupin vcpu='14' cpuset='15'/>
          <vcpupin vcpu='15' cpuset='31'/>
          <emulatorpin cpuset='0,16'/>
          <iothreadpin iothread='1' cpuset='0,16'/>
        </cputune>
        <os firmware='efi'>
          <type arch='x86_64' machine='pc-q35-11.1'>hvm</type>
          <firmware>
            <feature enabled='no' name='enrolled-keys'/>
            <feature enabled='yes' name='secure-boot'/>
          </firmware>
          <loader readonly='yes' secure='yes' type='pflash' format='raw'>/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd</loader>
          <nvram template='/run/libvirt/nix-ovmf/edk2-i386-vars.fd' templateFormat='raw' format='raw'>/var/lib/libvirt/qemu/nvram/${name}_VARS.fd</nvram>
          <boot dev='hd'/>
        </os>
        <features>
          <acpi/>
          <apic/>
          <hyperv mode='custom'>
            <relaxed state='on'/>
            <vapic state='on'/>
            <spinlocks state='on' retries='8191'/>
            <vpindex state='on'/>
            <runtime state='on'/>
            <synic state='on'/>
            <stimer state='on'/>
            <frequencies state='on'/>
            <tlbflush state='on'/>
            <ipi state='on'/>
            <avic state='on'/>
          </hyperv>
          <vmport state='off'/>
          <smm state='on'/>
        </features>
        <cpu mode='host-passthrough' check='none' migratable='on'>
          <topology sockets='1' dies='1' clusters='1' cores='8' threads='2'/>
          <feature policy='require' name='topoext'/>
        </cpu>
        <clock offset='localtime'>
          <timer name='rtc' tickpolicy='catchup'/>
          <timer name='pit' tickpolicy='delay'/>
          <timer name='hpet' present='no'/>
          <timer name='hypervclock' present='yes'/>
        </clock>
        <on_poweroff>destroy</on_poweroff>
        <on_reboot>restart</on_reboot>
        <on_crash>destroy</on_crash>
        <pm>
          <suspend-to-mem enabled='no'/>
          <suspend-to-disk enabled='no'/>
        </pm>
        <devices>
          <emulator>/run/libvirt/nix-emulators/qemu-system-x86_64</emulator>
          <disk type='file' device='disk' onehost:role='os-disk'>
            <target dev='sda' bus='sata'/>
            <address type='drive' controller='0' bus='0' target='0' unit='0'/>
          </disk>
          <controller type='usb' index='0' model='qemu-xhci' ports='15'>
            <address type='pci' domain='0x0000' bus='0x02' slot='0x00' function='0x0'/>
          </controller>
          <controller type='pci' index='0' model='pcie-root'/>
          <controller type='pci' index='1' model='pcie-root-port'>
            <model name='pcie-root-port'/>
            <target chassis='1' port='0x10'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x02' function='0x0' multifunction='on'/>
          </controller>
          <controller type='pci' index='2' model='pcie-root-port'>
            <model name='pcie-root-port'/>
            <target chassis='2' port='0x11'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x02' function='0x1'/>
          </controller>
          <controller type='pci' index='3' model='pcie-root-port'>
            <model name='pcie-root-port'/>
            <target chassis='3' port='0x12'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x02' function='0x2'/>
          </controller>
          <controller type='pci' index='4' model='pcie-root-port'>
            <model name='pcie-root-port'/>
            <target chassis='4' port='0x13'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x02' function='0x3'/>
          </controller>
          <controller type='pci' index='5' model='pcie-root-port'>
            <model name='pcie-root-port'/>
            <target chassis='5' port='0x14'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x02' function='0x4'/>
          </controller>
          <controller type='pci' index='6' model='pcie-root-port'>
            <model name='pcie-root-port'/>
            <target chassis='6' port='0x15'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x02' function='0x5'/>
          </controller>
          <controller type='pci' index='7' model='pcie-root-port'>
            <model name='pcie-root-port'/>
            <target chassis='7' port='0x16'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x02' function='0x6'/>
          </controller>
          <controller type='pci' index='8' model='pcie-root-port'>
            <model name='pcie-root-port'/>
            <target chassis='8' port='0x17'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x02' function='0x7'/>
          </controller>
          <controller type='sata' index='0'>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x1f' function='0x2'/>
          </controller>
          <controller type='virtio-serial' index='0'>
            <address type='pci' domain='0x0000' bus='0x03' slot='0x00' function='0x0'/>
          </controller>
          <interface type='network'>
            <mac address='${mac}'/>
            <source network='default'/>
            <model type='e1000e'/>
            <address type='pci' domain='0x0000' bus='0x01' slot='0x00' function='0x0'/>
          </interface>
          <serial type='pty'>
            <target type='isa-serial' port='0'>
              <model name='isa-serial'/>
            </target>
          </serial>
          <console type='pty'>
            <target type='serial' port='0'/>
          </console>
          <channel type='spicevmc'>
            <target type='virtio' name='com.redhat.spice.0'/>
            <address type='virtio-serial' controller='0' bus='0' port='1'/>
          </channel>
          <input type='tablet' bus='usb'>
            <address type='usb' bus='0' port='1'/>
          </input>
          <input type='mouse' bus='ps2'/>
          <input type='keyboard' bus='ps2'/>
          <input type='keyboard' bus='usb'>
            <address type='usb' bus='0' port='4'/>
          </input>
          <tpm model='tpm-crb'>
            <backend type='emulator' version='2.0'/>
          </tpm>
          <graphics type='spice' autoport='yes'>
            <listen type='address'/>
            <image compression='off'/>
          </graphics>
          <sound model='ich9'>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x1b' function='0x0'/>
          </sound>
          <audio id='1' type='spice'/>
          <video>
            <model type='qxl' ram='65536' vram='65536' vgamem='16384' heads='1' primary='yes'/>
            <address type='pci' domain='0x0000' bus='0x00' slot='0x01' function='0x0'/>
          </video>
          ${lib.optionalString (vfFunction != null) ''
            <hostdev mode='subsystem' type='pci' managed='yes'>
              <driver name='vfio'/>
              <source>
                <address domain='0x0000' bus='0x07' slot='0x00' function='${vfFunction}'/>
              </source>
              <address type='pci' domain='0x0000' bus='0x05' slot='0x00' function='0x0'/>
            </hostdev>
          ''}
          <watchdog model='itco' action='reset'/>
          <memballoon model='virtio'>
            <address type='pci' domain='0x0000' bus='0x04' slot='0x00' function='0x0'/>
          </memballoon>
        </devices>
        ${lib.optionalString (kvmfrDev != null) ''
          <qemu:commandline>
            <qemu:arg value='-device'/>
            <qemu:arg value='{&apos;driver&apos;:&apos;ivshmem-plain&apos;,&apos;id&apos;:&apos;shmem0&apos;,&apos;memdev&apos;:&apos;looking-glass&apos;}'/>
            <qemu:arg value='-object'/>
            <qemu:arg value='{&apos;qom-type&apos;:&apos;memory-backend-file&apos;,&apos;id&apos;:&apos;looking-glass&apos;,&apos;mem-path&apos;:&apos;${kvmfrDev}&apos;,&apos;size&apos;:134217728,&apos;share&apos;:true}'/>
          </qemu:commandline>
        ''}
      </domain>
    '';

  templateXmls = lib.mapAttrs (
    name: cfg:
    mkDomainVFIOTemplateXml {
      inherit name;
      inherit (cfg)
        uuid
        mac
        kvmfrDev
        vfFunction
        memory
        cpus
        ;
    }
  ) instances;

  manifest = {
    "$schema" = "https://middle-earth.internal/schemas/onehost.v1.json";
    version = "1.0";
    storage = {
      depot_store_dir = "/data/depot/virtualization/libvirt/store";
      depot_iso_dir = "/data/depot/virtualization/libvirt/iso";
      depot_backup_dir = "/data/depot/virtualization/libvirt/backup";
      nvram_dir = "/var/lib/libvirt/qemu/nvram";
      nvram_template = "/run/libvirt/nix-ovmf/edk2-i386-vars.fd";
      ovmf_code = "/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd";
    };
    flavors = lib.mapAttrs (_name: oemdrv: {
      oemdrv_path = "${oemdrv}";
      hash = builtins.substring 0 8 (baseNameOf oemdrv);
    }) oemdrvFlavors;
    instances = lib.mapAttrs (name: cfg: {
      uuid = cfg.uuid;
      template_xml = "${templateXmls.${name}}";
      image = cfg.image;
      pool = cfg.pool;
      autostart = cfg.autostart;
      lifecycle = cfg.lifecycle;
    }) instances;
  };

  manifestJson = pkgs.writeText "onehost.json" (builtins.toJSON manifest);

  makeOnehostApp =
    name: cmd:
    pkgs.writeShellApplication {
      inherit name;
      runtimeInputs = [
        onehostPkg
        pkgs.libvirt
        pkgs.qemu_kvm
      ];
      text = ''
        if [ "$(id -u)" -ne 0 ]; then
          exec sudo "$0" "$@"
        fi
        exec ${onehostPkg}/bin/onehost ${cmd} --manifest "${manifestJson}" "$@"
      '';
    };

  backupApp = pkgs.writeShellApplication {
    name = "onehost-backup";
    runtimeInputs = [
      onehostPkg
      pkgs.libvirt
      pkgs.qemu_kvm
    ];
    text = ''
      if [ "$(id -u)" -ne 0 ]; then
        exec sudo "$0" "$@"
      fi

      TARGET="''${1:-all}"
      if [ "$TARGET" = "all" ]; then
        ${lib.concatStringsSep "\n" (
          map (instanceName: ''
            echo "==> Backing up instance ${instanceName}..."
            ${onehostPkg}/bin/onehost backup "${instanceName}" --manifest "${manifestJson}" "''${@:2}"
          '') (builtins.attrNames instances)
        )}
      else
        exec ${onehostPkg}/bin/onehost backup "$@" --manifest "${manifestJson}"
      fi
    '';
  };

in
createFlakeModule {
  manifest = manifestJson;
  inherit
    templateXmls
    oemdrvFlavors
    mkOemdrv
    mkDomainVFIOTemplateXml
    ;
  apps = {
    plan = makeOnehostApp "onehost-plan" "plan";
    apply = makeOnehostApp "onehost-apply" "apply";
    destroy = makeOnehostApp "onehost-destroy" "destroy";
    backup = backupApp;
    restore = makeOnehostApp "onehost-restore" "restore";
    status = makeOnehostApp "onehost-status" "status";
    buildImage = makeOnehostApp "onehost-build-image" "image build";
    buildIso = windows.buildIsoApp;
  };
}
