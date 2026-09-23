{
  createFlakeModule,
  flake,
  nixpkgs,
  super,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};
  lib = pkgs.lib;

  windowsVersions = super."windows-versions";
  instances = flake.middle-earth.hosts.gandalf.virtualization.instances;

  normalizeLang =
    lang:
    let
      parts = lib.splitString "-" lang;
    in
    if builtins.length parts == 2 then
      "${lib.toLower (builtins.elemAt parts 0)}-${lib.toUpper (builtins.elemAt parts 1)}"
    else
      lang;

  instanceForVersion =
    verKey:
    lib.findFirst (
      name:
      let
        cfg = instances.${name};
        parts = lib.splitString "/" cfg.image;
      in
      (builtins.length parts >= 2) && (builtins.elemAt parts 1 == verKey)
    ) null (builtins.attrNames instances);

  hostnameForVersion =
    verKey:
    let
      inst = instanceForVersion verKey;
    in
    if inst != null then
      instances.${inst}.hostname or inst
    else
      "WIN11-VM";

  mkAutounattendXml =
    {
      edition ? "professional",
      language ? "en-us",
      computerName ? "WIN11-VM",
    }:
    let
      cleanEdition = lib.toLower edition;
      productKey =
        if cleanEdition == "professional" || cleanEdition == "pro" then
          "W269N-WFGWX-YVC9B-4J6C9-T83GX"
        else
          throw "Unsupported Windows edition: '${edition}'. Only 'professional' is currently supported.";
      langTag = normalizeLang language;
    in
    pkgs.writeText "autounattend-${cleanEdition}-${langTag}-${computerName}.xml" ''
      <?xml version="1.0" encoding="utf-8"?>
      <unattend xmlns="urn:schemas-microsoft-com:unattend" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
        <settings pass="windowsPE">
          <component name="Microsoft-Windows-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <DiskConfiguration>
              <Disk wcm:action="add">
                <DiskID>0</DiskID>
                <WillWipeDisk>true</WillWipeDisk>
                <CreatePartitions>
                  <CreatePartition wcm:action="add">
                    <Order>1</Order>
                    <Type>EFI</Type>
                    <Size>512</Size>
                  </CreatePartition>
                  <CreatePartition wcm:action="add">
                    <Order>2</Order>
                    <Type>MSR</Type>
                    <Size>16</Size>
                  </CreatePartition>
                  <CreatePartition wcm:action="add">
                    <Order>3</Order>
                    <Type>Primary</Type>
                    <Extend>true</Extend>
                  </CreatePartition>
                </CreatePartitions>
                <ModifyPartitions>
                  <ModifyPartition wcm:action="add">
                    <Order>1</Order>
                    <PartitionID>1</PartitionID>
                    <Format>FAT32</Format>
                    <Label>System</Label>
                  </ModifyPartition>
                  <ModifyPartition wcm:action="add">
                    <Order>2</Order>
                    <PartitionID>2</PartitionID>
                  </ModifyPartition>
                  <ModifyPartition wcm:action="add">
                    <Order>3</Order>
                    <PartitionID>3</PartitionID>
                    <Format>NTFS</Format>
                    <Label>Windows</Label>
                    <Letter>C</Letter>
                  </ModifyPartition>
                </ModifyPartitions>
              </Disk>
            </DiskConfiguration>
            <DynamicUpdate>
              <Enable>false</Enable>
              <WillShowUI>Never</WillShowUI>
            </DynamicUpdate>
            <ImageInstall>
              <OSImage>
                <InstallFrom>
                  <MetaData wcm:action="add">
                    <Key>/IMAGE/INDEX</Key>
                    <Value>1</Value>
                  </MetaData>
                </InstallFrom>
                <InstallTo>
                  <DiskID>0</DiskID>
                  <PartitionID>3</PartitionID>
                </InstallTo>
                <WillShowUI>OnError</WillShowUI>
              </OSImage>
            </ImageInstall>
            <UserData>
              <AcceptEula>true</AcceptEula>
              <ProductKey>
                <Key>${productKey}</Key>
                <WillShowUI>Never</WillShowUI>
              </ProductKey>
            </UserData>
            <RunSynchronous>
              <RunSynchronousCommand wcm:action="add">
                <Order>1</Order>
                <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassTPMCheck /t REG_DWORD /d 1 /f</Path>
              </RunSynchronousCommand>
              <RunSynchronousCommand wcm:action="add">
                <Order>2</Order>
                <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassSecureBootCheck /t REG_DWORD /d 1 /f</Path>
              </RunSynchronousCommand>
              <RunSynchronousCommand wcm:action="add">
                <Order>3</Order>
                <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassRAMCheck /t REG_DWORD /d 1 /f</Path>
              </RunSynchronousCommand>
              <RunSynchronousCommand wcm:action="add">
                <Order>4</Order>
                <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassStorageCheck /t REG_DWORD /d 1 /f</Path>
              </RunSynchronousCommand>
              <RunSynchronousCommand wcm:action="add">
                <Order>5</Order>
                <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v BypassCPUCheck /t REG_DWORD /d 1 /f</Path>
              </RunSynchronousCommand>
              <RunSynchronousCommand wcm:action="add">
                <Order>6</Order>
                <Path>cmd /c for %d in (D E F G H) do if exist %d:\ErrorHandler.cmd (mkdir C:\Windows\Setup\Scripts 2>nul &amp; copy /y %d:\ErrorHandler.cmd C:\Windows\Setup\Scripts\ErrorHandler.cmd)</Path>
              </RunSynchronousCommand>
            </RunSynchronous>
          </component>
          <component name="Microsoft-Windows-PnpCustomizationsWinPE" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <DriverPaths>
              <PathAndCredentials wcm:action="add" wcm:keyValue="1">
                <Path>E:\viostor\w11\amd64</Path>
              </PathAndCredentials>
              <PathAndCredentials wcm:action="add" wcm:keyValue="2">
                <Path>E:\vioscsi\w11\amd64</Path>
              </PathAndCredentials>
              <PathAndCredentials wcm:action="add" wcm:keyValue="3">
                <Path>E:\NetKVM\w11\amd64</Path>
              </PathAndCredentials>
            </DriverPaths>
          </component>
          <component name="Microsoft-Windows-International-Core-WinPE" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <SetupUILanguage>
              <UILanguage>${langTag}</UILanguage>
            </SetupUILanguage>
            <InputLocale>${langTag}</InputLocale>
            <SystemLocale>${langTag}</SystemLocale>
            <UILanguage>${langTag}</UILanguage>
            <UserLocale>${langTag}</UserLocale>
          </component>
        </settings>
        <settings pass="specialize">
          <component name="Microsoft-Windows-International-Core" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <InputLocale>${langTag}</InputLocale>
            <SystemLocale>${langTag}</SystemLocale>
            <UILanguage>${langTag}</UILanguage>
            <UserLocale>${langTag}</UserLocale>
          </component>
          <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <ComputerName>${computerName}</ComputerName>
            <TimeZone>W. Europe Standard Time</TimeZone>
          </component>
        </settings>
        <settings pass="oobeSystem">
          <component name="Microsoft-Windows-International-Core" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <InputLocale>${langTag}</InputLocale>
            <SystemLocale>${langTag}</SystemLocale>
            <UILanguage>${langTag}</UILanguage>
            <UserLocale>${langTag}</UserLocale>
          </component>
          <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <OOBE>
              <HideEULAPage>true</HideEULAPage>
              <HideLocalAccountScreen>true</HideLocalAccountScreen>
              <HideOEMRegistrationScreen>true</HideOEMRegistrationScreen>
              <HideOnlineAccountScreens>true</HideOnlineAccountScreens>
              <HideWirelessSetupInOOBE>true</HideWirelessSetupInOOBE>
              <NetworkLocation>Home</NetworkLocation>
              <ProtectYourPC>3</ProtectYourPC>
              <SkipUserOOBE>true</SkipUserOOBE>
              <SkipMachineOOBE>true</SkipMachineOOBE>
            </OOBE>
            <UserAccounts>
              <LocalAccounts>
                <LocalAccount wcm:action="add">
                  <Description>Administrator</Description>
                  <DisplayName>Admin</DisplayName>
                  <Group>Administrators</Group>
                  <Name>Admin</Name>
                  <Password>
                    <Value></Value>
                    <PlainText>true</PlainText>
                  </Password>
                </LocalAccount>
              </LocalAccounts>
            </UserAccounts>
            <AutoLogon>
              <Enabled>true</Enabled>
              <LogonCount>2</LogonCount>
              <Username>Admin</Username>
              <Password>
                <Value></Value>
                <PlainText>true</PlainText>
              </Password>
            </AutoLogon>
            <FirstLogonCommands>
              <SynchronousCommand wcm:action="add">
                <Order>1</Order>
                <CommandLine>powershell.exe -ExecutionPolicy Bypass -NoProfile -Command "Get-Volume | Where-Object { $_.DriveLetter } | ForEach-Object { $p = $_.DriveLetter + ':\provision.ps1'; if (Test-Path $p) { &amp; $p } }"</CommandLine>
                <Description>Run Golden Image Provisioning</Description>
              </SynchronousCommand>
            </FirstLogonCommands>
          </component>
        </settings>
      </unattend>
    '';

  autounattendXml = mkAutounattendXml { };

  mkSysprepXml =
    {
      language ? "en-us",
      computerName ? "WIN11-VM",
    }:
    let
      langTag = normalizeLang language;
    in
    pkgs.writeText "sysprep-${langTag}-${computerName}.xml" ''
      <?xml version="1.0" encoding="utf-8"?>
      <unattend xmlns="urn:schemas-microsoft-com:unattend" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
        <settings pass="specialize">
          <component name="Microsoft-Windows-International-Core" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <InputLocale>${langTag}</InputLocale>
            <SystemLocale>${langTag}</SystemLocale>
            <UILanguage>${langTag}</UILanguage>
            <UserLocale>${langTag}</UserLocale>
          </component>
          <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <ComputerName>${computerName}</ComputerName>
            <TimeZone>W. Europe Standard Time</TimeZone>
          </component>
        </settings>
        <settings pass="oobeSystem">
          <component name="Microsoft-Windows-International-Core" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <InputLocale>${langTag}</InputLocale>
            <SystemLocale>${langTag}</SystemLocale>
            <UILanguage>${langTag}</UILanguage>
            <UserLocale>${langTag}</UserLocale>
          </component>
          <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
            <OOBE>
              <HideEULAPage>true</HideEULAPage>
              <HideLocalAccountScreen>true</HideLocalAccountScreen>
              <HideOEMRegistrationScreen>true</HideOEMRegistrationScreen>
              <HideOnlineAccountScreens>true</HideOnlineAccountScreens>
              <HideWirelessSetupInOOBE>true</HideWirelessSetupInOOBE>
              <NetworkLocation>Home</NetworkLocation>
              <ProtectYourPC>3</ProtectYourPC>
              <SkipUserOOBE>true</SkipUserOOBE>
              <SkipMachineOOBE>true</SkipMachineOOBE>
            </OOBE>
            <AutoLogon>
              <Enabled>true</Enabled>
              <LogonCount>1</LogonCount>
              <Username>Admin</Username>
              <Password>
                <Value></Value>
                <PlainText>true</PlainText>
              </Password>
            </AutoLogon>
          </component>
        </settings>
      </unattend>
    '';

  errorHandlerCmd = pkgs.writeText "ErrorHandler.cmd" ''
    @echo off
    echo ============================================================ > COM1
    echo [ERROR] Windows Setup encountered a fatal error! >> COM1
    echo ============================================================ >> COM1
    if exist C:\Windows\Panther\setuperr.log (
      echo --- C:\Windows\Panther\setuperr.log --- >> COM1
      type C:\Windows\Panther\setuperr.log >> COM1
    )
    if exist C:\Windows\Panther\UnattendGC\setuperr.log (
      echo --- C:\Windows\Panther\UnattendGC\setuperr.log --- >> COM1
      type C:\Windows\Panther\UnattendGC\setuperr.log >> COM1
    )
    if exist C:\Windows\Panther\setupact.log (
      echo --- Tail of C:\Windows\Panther\setupact.log --- >> COM1
      powershell -Command "Get-Content C:\Windows\Panther\setupact.log -Tail 60" >> COM1
    )
    echo ============================================================ >> COM1
  '';

  provisionPs1 = pkgs.writeText "provision.ps1" ''
    $ErrorActionPreference = "Continue"

    function Log($msg) {
        Write-Host $msg
        try { [System.IO.File]::AppendAllText("\\.\COM1", "$msg`r`n") } catch {}
    }

    Log "==> [1/4] Installing VirtIO Guest Tools..."
    $virtioVol = Get-Volume | Where-Object { $_.DriveLetter -and (Test-Path ($_.DriveLetter + ':\virtio-win-guest-tools.exe')) } | Select-Object -First 1
    if ($virtioVol) {
        Log "--> Found VirtIO media on $($virtioVol.DriveLetter):, launching installer..."
        Start-Process -FilePath ($virtioVol.DriveLetter + ':\virtio-win-guest-tools.exe') -ArgumentList "/install", "/passive", "/norestart" -Wait
        Log "--> VirtIO Guest Tools installation complete."
    }

    Log "==> [2/4] Installing Looking Glass Host (if provided in OEMDRV)..."
    $scriptVol = Get-Volume | Where-Object { $_.DriveLetter -and (Test-Path ($_.DriveLetter + ':\provision.ps1')) } | Select-Object -First 1
    if ($scriptVol) {
        $scriptDrive = $scriptVol.DriveLetter
        $lgZip = $scriptDrive + ':\looking-glass-host.zip'
        $lgExe = $scriptDrive + ':\looking-glass-host-setup.exe'

        if (Test-Path $lgExe) {
            Start-Process -FilePath $lgExe -ArgumentList "/S" -Wait
            Log "--> Looking Glass Host installed via setup.exe."
        } elseif (Test-Path $lgZip) {
            $dest = "C:\Program Files\Looking Glass (host)"
            New-Item -ItemType Directory -Path $dest -Force | Out-Null
            Expand-Archive -Path $lgZip -DestinationPath $dest -Force
            $hostBin = Join-Path $dest "looking-glass-host.exe"
            if (Test-Path $hostBin) {
                Start-Process -FilePath $hostBin -ArgumentList "--install" -Wait
                Log "--> Looking Glass Host service installed."
            }
        }
    }

    Log "==> [3/4] Tuning system power and service settings..."
    powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c
    powercfg /change standby-timeout-ac 0
    powercfg /change monitor-timeout-ac 0
    powercfg /hibernate off

    Set-Service -Name wuauserv -StartupType Disabled -ErrorAction SilentlyContinue

    Log "==> [4/4] Generalizing golden image with Sysprep..."
    if ($scriptVol) {
        $sysprepXml = $scriptVol.DriveLetter + ':\sysprep.xml'
        if (Test-Path $sysprepXml) {
            $sysprepDst = "$env:SystemRoot\System32\Sysprep\unattend.xml"
            Log "--> Installing unattended answer file to $sysprepDst for post-sysprep OOBE automation..."
            Copy-Item -Path $sysprepXml -Destination $sysprepDst -Force
        }
    }
    Start-Sleep -Seconds 5
    Start-Process -FilePath "$env:SystemRoot\System32\Sysprep\sysprep.exe" -ArgumentList "/generalize", "/oobe", "/shutdown", "/quiet" -Wait
  '';

  uupEnv = pkgs.buildFHSEnv {
    name = "uup-env";
    targetPkgs = pkgs: [
      pkgs.bash
      pkgs.coreutils
      pkgs.gnused
      pkgs.gnugrep
      pkgs.gawk
      pkgs.findutils
      pkgs.diffutils
      pkgs.which
      pkgs.aria2
      pkgs.cabextract
      pkgs.wimlib
      pkgs.chntpw
      pkgs.cdrkit
      pkgs.curl
      pkgs.jq
      pkgs.unzip
      pkgs.util-linux
    ];
    runScript = "bash";
  };

  versionLookupScript = lib.concatStringsSep "\n" (
    lib.mapAttrsToList (
      verKey: verData:
      let
        compName = hostnameForVersion verKey;
      in
      ''
        "${verKey}")
          UUP_ID="${verData.uupId}"
          WIN_EDITION="${verData.edition}"
          WIN_LANG="${verData.language}"
          AUTOUNATTEND_XML="${mkAutounattendXml { inherit (verData) edition language; computerName = compName; }}"
          SYSPREP_XML="${mkSysprepXml { inherit (verData) language; computerName = compName; }}"
          ;;
      ''
    ) windowsVersions.versions
  );

  buildApp = pkgs.writeShellApplication {
    name = "build-windows-image";
    runtimeInputs = [
      pkgs.qemu_kvm
      pkgs.swtpm
      pkgs.cdrkit
      pkgs.curl
      pkgs.jq
      pkgs.coreutils
      pkgs.gnused
      pkgs.util-linux
      pkgs.unzip
      pkgs.socat
      uupEnv
    ];
    text = ''
      if [ -z "''${1:-}" ]; then
        echo "ERROR: Missing required IMAGE_TAG argument."
        echo "Usage: build-windows-image <os>/<version>/<profile>/<revision> [hostname]"
        echo "Supported versions: ${builtins.concatStringsSep ", " (builtins.attrNames windowsVersions.versions)}"
        exit 1
      fi

      IMAGE_TAG="$1"
      VM_HOSTNAME="''${2:-}"
      IFS='/' read -r OS WIN_VERSION PROFILE REVISION <<< "$IMAGE_TAG"

      if [ -z "$OS" ] || [ -z "$WIN_VERSION" ] || [ -z "$PROFILE" ] || [ -z "$REVISION" ]; then
        echo "ERROR: Malformed IMAGE_TAG '$IMAGE_TAG'. Must follow format: <os>/<version>/<profile>/<revision> [hostname]"
        echo "Example: win11/26300.9457.pro.en-us/looking-glass/v2"
        exit 1
      fi

      case "$WIN_VERSION" in
        ${versionLookupScript}
        *)
          echo "ERROR: Unknown Windows version '$WIN_VERSION'."
          echo "Supported versions: ${builtins.concatStringsSep ", " (builtins.attrNames windowsVersions.versions)}"
          exit 1
          ;;
      esac

      UUID_SHORT="''${UUP_ID:0:8}"
      DEPOT_ISO_DIR="/data/depot/virtualization/libvirt/iso"
      DEPOT_STORE_DIR="/data/depot/virtualization/libvirt/store"
      WIN11_ISO="$DEPOT_ISO_DIR/win11-$WIN_VERSION-$UUID_SHORT.iso"
      BUILD_IMAGE="/var/lib/libvirt/images/win11-builder-$$.qcow2"
      OUTPUT_IMAGE="$DEPOT_STORE_DIR/$OS-$WIN_VERSION-$UUID_SHORT-$PROFILE-$REVISION.qcow2"
      VIRTIO_ISO="${pkgs.virtio-win.src}"
      OVMF_CODE="${pkgs.OVMF.fd}/FV/OVMF_CODE.fd"
      OVMF_VARS_TEMPLATE="${pkgs.OVMF.fd}/FV/OVMF_VARS.fd"

      mkdir -p "$DEPOT_ISO_DIR" "$DEPOT_STORE_DIR" "/var/lib/libvirt/images"

      if [ -f "$OUTPUT_IMAGE" ]; then
        echo "ERROR: Golden image already exists at $OUTPUT_IMAGE."
        echo "The image store is strictly immutable. If you need modifications, bump the revision tag (e.g. v2)."
        exit 1
      fi

      echo "============================================================"
      echo "==> [Builder] Building Windows Golden Image"
      echo "    Tag:         $IMAGE_TAG"
      echo "    OS:          $OS"
      echo "    Version:     $WIN_VERSION"
      echo "    UUP ID:      $UUP_ID"
      echo "    Language:    $WIN_LANG"
      echo "    Edition:     $WIN_EDITION"
      echo "    Profile:     $PROFILE"
      echo "    Revision:    $REVISION"
      echo "    ISO Path:    $WIN11_ISO"
      echo "    Output Disk: $OUTPUT_IMAGE"
      echo "============================================================"

      # Step 1: Ensure Windows 11 ISO is present in depot
      if [ ! -f "$WIN11_ISO" ] || [ ! -s "$WIN11_ISO" ]; then
        echo "==> [1/5] Windows 11 ISO not found in depot ($WIN11_ISO)."
        echo "--> Fetching and building UUP ID: $UUP_ID via UUP dump..."

        UUP_BUILD_DIR="$(mktemp -d /tmp/win11-uup-XXXXXX)"
        UUP_ZIP="$UUP_BUILD_DIR/uup.zip"

        echo "--> Downloading UUP dump converter package for ID $UUP_ID..."
        curl -sSfL "https://uupdump.net/get.php?id=''${UUP_ID}&pack=''${WIN_LANG}&edition=''${WIN_EDITION}&autodl=2" -o "$UUP_ZIP"

        unzip -q -o "$UUP_ZIP" -d "$UUP_BUILD_DIR"

        echo "--> Running UUP download and ISO creation inside FHS environment (streaming official update files from Microsoft)..."
        (
          cd "$UUP_BUILD_DIR"
          chmod +x ./uup_download_linux.sh
          ${uupEnv}/bin/uup-env -c "./uup_download_linux.sh"
        )

        GENERATED_ISO="$(find "$UUP_BUILD_DIR" -maxdepth 2 -type f \( -name "*.ISO" -o -name "*.iso" \) | head -n 1)"
        if [ -z "$GENERATED_ISO" ] || [ ! -s "$GENERATED_ISO" ]; then
          echo "ERROR: Failed to generate Windows 11 ISO via UUP dump."
          rm -rf "$UUP_BUILD_DIR"
          exit 1
        fi

        echo "--> Moving generated ISO to $WIN11_ISO..."
        mv "$GENERATED_ISO" "$WIN11_ISO"
        rm -rf "$UUP_BUILD_DIR"
        echo "--> Successfully created and cached Windows 11 ISO at $WIN11_ISO"
      else
        echo "==> [1/5] Using cached Windows 11 ISO at $WIN11_ISO"
      fi

      # Step 2: Prepare secondary OEMDRV media containing unattended answer file & scripts
      echo "==> [2/5] Generating unattended installation media (Profile: $PROFILE)..."
      UNATTEND_DIR="$(mktemp -d /tmp/win11-unattend-XXXXXX)"
      UNATTEND_ISO="$(mktemp /tmp/win11-unattend-iso-XXXXXX.iso)"
      SWTPM_DIR="$(mktemp -d /tmp/win11-swtpm-XXXXXX)"
      OVMF_VARS="$(mktemp /tmp/win11-ovmf-vars-XXXXXX.fd)"
      MONITOR_SOCK="/tmp/win11-monitor-$$.sock"

      cleanup() {
        echo "==> Cleaning up ephemeral build artifacts..."
        if [ -n "''${KEY_SENDER_PID:-}" ]; then
          kill "$KEY_SENDER_PID" 2>/dev/null || true
        fi
        if [ -n "''${SWTPM_PID:-}" ] && kill -0 "$SWTPM_PID" 2>/dev/null; then
          kill "$SWTPM_PID" 2>/dev/null || true
        fi
        rm -rf "$UNATTEND_DIR" "$UNATTEND_ISO" "$SWTPM_DIR" "$OVMF_VARS" "$MONITOR_SOCK"
        if [ -f "$BUILD_IMAGE" ]; then
          rm -f "$BUILD_IMAGE"
        fi
      }
      trap cleanup EXIT

      cp "$AUTOUNATTEND_XML" "$UNATTEND_DIR/autounattend.xml"
      cp "$SYSPREP_XML" "$UNATTEND_DIR/sysprep.xml"
      if [ -n "$VM_HOSTNAME" ]; then
        echo "--> Applying instance hostname '$VM_HOSTNAME' to unattended answer files..."
        sed -i "s|<ComputerName>.*</ComputerName>|<ComputerName>$VM_HOSTNAME</ComputerName>|g" "$UNATTEND_DIR/sysprep.xml" "$UNATTEND_DIR/autounattend.xml"
      fi
      cp "${provisionPs1}" "$UNATTEND_DIR/provision.ps1"
      cp "${errorHandlerCmd}" "$UNATTEND_DIR/ErrorHandler.cmd"

      if [ "$PROFILE" = "looking-glass" ]; then
        echo "--> Fetching Looking Glass Host B7 installer for looking-glass profile..."
        curl -sSfL -A "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/119.0" "https://looking-glass.io/artifact/B7/host" -o "$UNATTEND_DIR/looking-glass-host.zip" || true
      fi

      genisoimage -o "$UNATTEND_ISO" -J -r -V "OEMDRV" "$UNATTEND_DIR" >/dev/null 2>&1

      # Step 3: Initialize TPM emulator and UEFI variable store
      echo "==> [3/5] Starting swtpm TPM 2.0 emulator..."
      cp "$OVMF_VARS_TEMPLATE" "$OVMF_VARS"
      chmod 0644 "$OVMF_VARS"

      swtpm socket \
        --tpmstate dir="$SWTPM_DIR" \
        --ctrl type=unixio,path="$SWTPM_DIR/swtpm-sock" \
        --tpm2 &
      SWTPM_PID=$!
      sleep 1

      echo "--> Creating 64G ephemeral build disk ($BUILD_IMAGE)..."
      qemu-img create -f qcow2 "$BUILD_IMAGE" 64G

      # Step 4: Boot headless QEMU for automated installation and sysprep
      echo "==> [4/5] Launching headless QEMU builder VM (VNC on :99)..."
      echo "--> Windows 11 installation and provisioning will proceed automatically."
      echo "--> Once complete, Sysprep will shut down the VM."

      (
        # Continuously send Enter and Space to QEMU monitor during boot to skip "Press any key to boot from CD or DVD..."
        for _ in $(seq 1 40); do
          sleep 0.5
          if [ -S "$MONITOR_SOCK" ]; then
            echo "sendkey ret" | socat -,shut-down unix-connect:"$MONITOR_SOCK" >/dev/null 2>&1 || true
            echo "sendkey spc" | socat -,shut-down unix-connect:"$MONITOR_SOCK" >/dev/null 2>&1 || true
          fi
        done
      ) &
      KEY_SENDER_PID=$!

      qemu-system-x86_64 \
        -enable-kvm \
        -cpu host,hv_relaxed,hv_spinlocks=0x1fff,hv_vapic,hv_time \
        -smp 8,sockets=1,cores=8,threads=1 \
        -m 8192 \
        -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
        -drive if=pflash,format=raw,file="$OVMF_VARS" \
        -chardev socket,id=chrtpm,path="$SWTPM_DIR/swtpm-sock" \
        -tpmdev emulator,id=tpm0,chardev=chrtpm \
        -device tpm-tis,tpmdev=tpm0 \
        -drive file="$BUILD_IMAGE",if=virtio,format=qcow2,cache=none \
        -drive file="$WIN11_ISO",media=cdrom,readonly=on \
        -drive file="$VIRTIO_ISO",media=cdrom,readonly=on \
        -drive file="$UNATTEND_ISO",media=cdrom,readonly=on \
        -net nic,model=virtio -net user \
        -monitor unix:"$MONITOR_SOCK",server,nowait \
        -vnc :99 \
        -nographic

      echo "==> [5/5] QEMU builder VM shut down cleanly."
      echo "--> Compressing and archiving golden master to $OUTPUT_IMAGE..."
      qemu-img convert -O qcow2 -c "$BUILD_IMAGE" "$OUTPUT_IMAGE"
      chmod 0444 "$OUTPUT_IMAGE" || true

      echo "==> Successfully created, generalized, and archived Windows 11 golden image: $OUTPUT_IMAGE"
    '';
  };

  mkWin11Domain =
    {
      name,
      uuid,
      mac,
      diskPath,
      nvramPath,
      kvmfrDev,
      vfFunction,
      memory ? 16777216,
      cpus ? 16,
    }:
    pkgs.writeText "${name}-domain.xml" ''
      <domain type='kvm' xmlns:qemu='http://libvirt.org/schemas/domain/qemu/1.0'>
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
          <nvram template='/run/libvirt/nix-ovmf/edk2-i386-vars.fd' templateFormat='raw' format='raw'>${nvramPath}</nvram>
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
          <disk type='file' device='disk'>
            <driver name='qemu' type='qcow2'/>
            <source file='${diskPath}'/>
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
          <hostdev mode='subsystem' type='pci' managed='yes'>
            <driver name='vfio'/>
            <source>
              <address domain='0x0000' bus='0x07' slot='0x00' function='${vfFunction}'/>
            </source>
            <address type='pci' domain='0x0000' bus='0x05' slot='0x00' function='0x0'/>
          </hostdev>
          <watchdog model='itco' action='reset'/>
          <memballoon model='virtio'>
            <address type='pci' domain='0x0000' bus='0x04' slot='0x00' function='0x0'/>
          </memballoon>
        </devices>
        <qemu:commandline>
          <qemu:arg value='-device'/>
          <qemu:arg value='{&apos;driver&apos;:&apos;ivshmem-plain&apos;,&apos;id&apos;:&apos;shmem0&apos;,&apos;memdev&apos;:&apos;looking-glass&apos;}'/>
          <qemu:arg value='-object'/>
          <qemu:arg value='{&apos;qom-type&apos;:&apos;memory-backend-file&apos;,&apos;id&apos;:&apos;looking-glass&apos;,&apos;mem-path&apos;:&apos;${kvmfrDev}&apos;,&apos;size&apos;:134217728,&apos;share&apos;:true}'/>
        </qemu:commandline>
      </domain>
    '';

  domainXmls = lib.mapAttrs (
    name: cfg:
    mkWin11Domain {
      inherit name;
      inherit (cfg)
        uuid
        mac
        kvmfrDev
        vfFunction
        ;
      memory = cfg.memory or 16777216;
      cpus = cfg.cpus or 16;
      diskPath = "/var/lib/libvirt/images/${name}.qcow2";
      nvramPath = "/var/lib/libvirt/qemu/nvram/${name}_VARS.fd";
    }
  ) instances;

  provisionInstance =
    name: cfg:
    let
      parts = lib.splitString "/" cfg.image;
      os = builtins.elemAt parts 0;
      verKey = builtins.elemAt parts 1;
      profile = builtins.elemAt parts 2;
      revision = builtins.elemAt parts 3;
      v = windowsVersions.resolveVersion verKey;
      uuidShort = builtins.substring 0 8 v.uupId;
      tagKey = "${os}-${verKey}-${uuidShort}-${profile}-${revision}";
      xmlFile = domainXmls.${name};
      depotMaster = "/data/depot/virtualization/libvirt/store/${tagKey}.qcow2";
      localBase = "/var/lib/libvirt/images/${tagKey}.qcow2";
      diskPath = "/var/lib/libvirt/images/${name}.qcow2";
      nvramPath = "/var/lib/libvirt/qemu/nvram/${name}_VARS.fd";
    in
    ''provision_one "${name}" "${xmlFile}" "${cfg.image}" "${depotMaster}" "${localBase}" "${diskPath}" "${nvramPath}"'';

  provisionAllScript = lib.concatStringsSep "\n" (
    lib.mapAttrsToList (name: cfg: provisionInstance name cfg) instances
  );

  provisionTargetsScript = lib.concatStringsSep "\n" (
    lib.mapAttrsToList (
      name: cfg: ''
        "${name}")
          ${provisionInstance name cfg}
          ;;
      ''
    ) instances
  );

  availableTargets = builtins.concatStringsSep ", " (builtins.attrNames instances);

  provisionApp = pkgs.writeShellApplication {
    name = "provision-windows-vm";
    runtimeInputs = [
      pkgs.qemu_kvm
      pkgs.coreutils
      pkgs.libvirt
      buildApp
    ];
    text = ''
      TARGET="all"
      RESET_OVERLAY=0

      for arg in "$@"; do
        case "$arg" in
          --reset)
            RESET_OVERLAY=1
            ;;
          -*)
            echo "Unknown option: $arg"
            echo "Usage: provision-windows-vm [vm-name|all] [--reset]"
            exit 1
            ;;
          *)
            TARGET="$arg"
            ;;
        esac
      done

      provision_one() {
        VM_NAME="$1"
        XML_FILE="$2"
        IMAGE_TAG="$3"
        DEPOT_MASTER="$4"
        LOCAL_BASE="$5"
        DISK_PATH="$6"
        NVRAM_PATH="$7"
        NVRAM_TEMPLATE="/run/libvirt/nix-ovmf/edk2-i386-vars.fd"

        echo "============================================================"
        echo "==> [Provision] Windows VM: $VM_NAME"
        echo "    Target Image: $IMAGE_TAG"
        echo "    Depot Store:  $DEPOT_MASTER"
        echo "    Local Base:   $LOCAL_BASE"
        echo "    Disk Path:    $DISK_PATH"
        echo "============================================================"

        # 1. Check if depot master exists (JIT build if missing)
        if [ ! -f "$DEPOT_MASTER" ]; then
          echo "--> Golden master not found in depot store ($DEPOT_MASTER)."
          echo "--> Initiating just-in-time build via build-windows-image..."
          "${buildApp}/bin/build-windows-image" "$IMAGE_TAG" "$VM_NAME"
          if [ ! -f "$DEPOT_MASTER" ]; then
            echo "ERROR: build-windows-image completed but $DEPOT_MASTER was not created."
            exit 1
          fi
        fi

        # 2. Check if base image is cached in local Libvirt storage pool
        if [ ! -f "$LOCAL_BASE" ]; then
          echo "--> Syncing golden master to local Libvirt storage pool ($LOCAL_BASE)..."
          cp "$DEPOT_MASTER" "$LOCAL_BASE"
          chmod 0444 "$LOCAL_BASE" || true
          echo "--> Local base image ready."
        fi

        # 3. Handle disk overlay
        if [ "$RESET_OVERLAY" -eq 1 ] && [ -f "$DISK_PATH" ]; then
          echo "--> Reset requested: removing existing overlay $DISK_PATH..."
          rm -f "$DISK_PATH"
        fi

        if [ -f "$DISK_PATH" ]; then
          echo "--> Disk $DISK_PATH already exists, keeping existing overlay."
        else
          echo "--> Creating fast CoW overlay backed by $LOCAL_BASE..."
          qemu-img create -f qcow2 -F qcow2 -b "$LOCAL_BASE" "$DISK_PATH"
          chmod 0660 "$DISK_PATH" || true
          echo "--> Created $DISK_PATH"
        fi

        # 4. Handle NVRAM
        if [ -f "$NVRAM_PATH" ]; then
          echo "--> NVRAM $NVRAM_PATH already exists, keeping existing variables."
        else
          if [ -f "$NVRAM_TEMPLATE" ]; then
            echo "--> Copying UEFI NVRAM template to $NVRAM_PATH..."
            cp "$NVRAM_TEMPLATE" "$NVRAM_PATH"
            chmod 0660 "$NVRAM_PATH" || true
          fi
        fi

        # 5. Define domain in Libvirt
        echo "--> Registering domain in Libvirt (virsh define)..."
        virsh -c qemu:///system define "$XML_FILE"
        echo "==> Successfully provisioned and registered $VM_NAME in Libvirt."
      }

      case "$TARGET" in
        all)
          ${provisionAllScript}
          ;;
        ${provisionTargetsScript}
        *)
          echo "ERROR: Unknown target '$TARGET'."
          echo "Available targets: all, ${availableTargets}"
          exit 1
          ;;
      esac
    '';
  };

  backupAllScript = lib.concatStringsSep "\n" (
    map (name: ''backup_one "${name}"'') (builtins.attrNames instances)
  );

  backupApp = pkgs.writeShellApplication {
    name = "backup-windows-vm";
    runtimeInputs = [
      pkgs.libvirt
      pkgs.qemu_kvm
      pkgs.coreutils
      pkgs.gnugrep
      pkgs.gawk
      pkgs.jq
    ];
    text = ''
      if [ "$(id -u)" -ne 0 ]; then
        exec sudo "$0" "$@"
      fi

      TARGET="''${1:-all}"
      BACKUP_BASE="/data/depot/virtualization/libvirt/backup"

      backup_one() {
        VM_NAME="$1"
        if ! virsh -c qemu:///system dominfo "$VM_NAME" >/dev/null 2>&1; then
          echo "WARNING: Domain $VM_NAME is not defined in Libvirt. Skipping."
          return 0
        fi

        DISK_SRC="$(virsh -c qemu:///system domblklist "$VM_NAME" --details 2>/dev/null | awk '$1 == "file" && $2 == "disk" {print $4}' | head -n 1)"
        if [ -z "$DISK_SRC" ] || [ ! -f "$DISK_SRC" ]; then
          echo "WARNING: Disk for $VM_NAME not found ($DISK_SRC). Skipping."
          return 0
        fi

        TARGET_DEV="$(virsh -c qemu:///system domblklist "$VM_NAME" --details 2>/dev/null | awk '$1 == "file" && $2 == "disk" {print $3}' | head -n 1)"
        [ -z "$TARGET_DEV" ] && TARGET_DEV="sda"

        VM_BACKUP_DIR="$BACKUP_BASE/$VM_NAME"
        STAGING_DISK="$VM_BACKUP_DIR/$VM_NAME.qcow2"
        TMP_DISK="$VM_BACKUP_DIR/$VM_NAME.qcow2.tmp"

        mkdir -p "$VM_BACKUP_DIR"
        echo "==> [Backup] Starting backup for $VM_NAME (disk: $DISK_SRC)..."

        BACKING_FILE="$(qemu-img info -U --output=json "$DISK_SRC" | jq -r '."backing-filename" // empty')"
        BACKING_FMT="$(qemu-img info -U --output=json "$DISK_SRC" | jq -r '."backing-filename-format" // empty')"

        IS_RUNNING=0
        if virsh -c qemu:///system dominfo "$VM_NAME" 2>/dev/null | grep -E "State:.*running" >/dev/null 2>&1; then
          IS_RUNNING=1
        fi

        if [ "$IS_RUNNING" -eq 1 ]; then
          echo "    VM is RUNNING. Initiating zero-downtime live external snapshot..."
          SNAP_OVERLAY="$DISK_SRC.snap"

          virsh -c qemu:///system snapshot-create-as \
            --domain "$VM_NAME" \
            --name "backup-$VM_NAME" \
            --diskspec "$TARGET_DEV,file=$SNAP_OVERLAY" \
            --disk-only --atomic --no-metadata >/dev/null 2>&1

          if [ -n "$BACKING_FILE" ]; then
            echo "    Live overlay active. Extracting consistent thin overlay (backing: $BACKING_FILE)..."
            qemu-img convert -U -O qcow2 -c -B "$BACKING_FILE" -F "$BACKING_FMT" "$DISK_SRC" "$TMP_DISK"
          else
            echo "    Live overlay active. Extracting flat disk image (no backing file)..."
            qemu-img convert -U -O qcow2 -c "$DISK_SRC" "$TMP_DISK"
          fi

          echo "    Committing delta back to base disk..."
          virsh -c qemu:///system blockcommit "$VM_NAME" "$TARGET_DEV" \
            --base "$DISK_SRC" \
            --top "$SNAP_OVERLAY" \
            --active --pivot >/dev/null 2>&1
          rm -f "$SNAP_OVERLAY"
        else
          if [ -n "$BACKING_FILE" ]; then
            echo "    VM is SHUT OFF. Extracting thin overlay (backing: $BACKING_FILE)..."
            qemu-img convert -U -O qcow2 -c -B "$BACKING_FILE" -F "$BACKING_FMT" "$DISK_SRC" "$TMP_DISK"
          else
            echo "    VM is SHUT OFF. Copying flat disk directly..."
            qemu-img convert -U -O qcow2 -c "$DISK_SRC" "$TMP_DISK"
          fi
        fi

        mv -f "$TMP_DISK" "$STAGING_DISK"

        if [ -n "$BACKING_FILE" ]; then
          echo "$BACKING_FILE" > "$VM_BACKUP_DIR/backing-file.txt"
        fi

        NVRAM_SRC="$(virsh -c qemu:///system dumpxml "$VM_NAME" 2>/dev/null | grep -oP '<nvram[^>]*>\K[^<]+' || true)"
        if [ -n "$NVRAM_SRC" ] && [ -f "$NVRAM_SRC" ]; then
          cp -f "$NVRAM_SRC" "$VM_BACKUP_DIR/nvram.fd"
        fi
        virsh -c qemu:///system dumpxml "$VM_NAME" > "$VM_BACKUP_DIR/domain.xml" 2>/dev/null || true

        echo "==> [Backup] Successfully staged thin backup for $VM_NAME in $VM_BACKUP_DIR"
      }

      case "$TARGET" in
        all)
          ${backupAllScript}
          ;;
        *)
          backup_one "$TARGET"
          ;;
      esac
    '';
  };

in
createFlakeModule {
  inherit
    mkAutounattendXml
    autounattendXml
    mkSysprepXml
    provisionPs1
    errorHandlerCmd
    buildApp
    provisionApp
    backupApp
    uupEnv
    domainXmls
    ;
}
