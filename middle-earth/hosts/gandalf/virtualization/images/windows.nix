{
  createFlakeModule,
  nixpkgs,
  super,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};
  lib = pkgs.lib;

  windowsVersions = super."windows-versions";

  normalizeLang =
    lang:
    let
      parts = lib.splitString "-" lang;
    in
    if builtins.length parts == 2 then
      "${lib.toLower (builtins.elemAt parts 0)}-${lib.toUpper (builtins.elemAt parts 1)}"
    else
      lang;

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

  buildIsoApp = pkgs.writeShellApplication {
    name = "onehost-build-iso";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.curl
      pkgs.unzip
      pkgs.findutils
      uupEnv
    ];
    text = ''
      VERSION_KEY="''${1:-}"
      if [ -z "$VERSION_KEY" ]; then
        echo "Usage: onehost-build-iso <version-key>"
        echo "Supported versions: ${builtins.concatStringsSep ", " (builtins.attrNames windowsVersions.versions)}"
        exit 1
      fi

      case "$VERSION_KEY" in
        ${lib.concatStringsSep "\n" (
          lib.mapAttrsToList (
            verKey: verData: ''
              "${verKey}")
                UUP_ID="${verData.uupId}"
                WIN_EDITION="${verData.edition}"
                WIN_LANG="${verData.language}"
                ;;
            ''
          ) windowsVersions.versions
        )}
        *)
          echo "ERROR: Unsupported Windows version '$VERSION_KEY'."
          echo "Supported versions: ${builtins.concatStringsSep ", " (builtins.attrNames windowsVersions.versions)}"
          exit 1
          ;;
      esac

      DEPOT_ISO_DIR="/data/depot/virtualization/libvirt/iso"
      OUTPUT_ISO="$DEPOT_ISO_DIR/win11-$VERSION_KEY.iso"

      if [ -f "$OUTPUT_ISO" ] && [ -s "$OUTPUT_ISO" ]; then
        echo "==> Windows 11 ISO already exists at $OUTPUT_ISO (skipping download)."
        exit 0
      fi

      mkdir -p "$DEPOT_ISO_DIR"
      BUILD_DIR="$(mktemp -d /tmp/win11-uup-XXXXXX)"
      UUP_ZIP="$BUILD_DIR/uup.zip"

      cleanup() {
        rm -rf "$BUILD_DIR"
      }
      trap cleanup EXIT

      echo "==> [1/3] Downloading UUP dump converter package for $VERSION_KEY (ID: $UUP_ID)..."
      curl -sSfL "https://uupdump.net/get.php?id=''${UUP_ID}&pack=''${WIN_LANG}&edition=''${WIN_EDITION}&autodl=2" -o "$UUP_ZIP"
      unzip -q -o "$UUP_ZIP" -d "$BUILD_DIR"

      echo "==> [2/3] Building Windows 11 ISO inside isolated FHS environment..."
      (
        cd "$BUILD_DIR"
        chmod +x ./uup_download_linux.sh
        ${uupEnv}/bin/uup-env -c "./uup_download_linux.sh"
      )

      GENERATED_ISO="$(find "$BUILD_DIR" -maxdepth 2 -type f \( -name "*.ISO" -o -name "*.iso" \) | head -n 1)"
      if [ -z "$GENERATED_ISO" ] || [ ! -s "$GENERATED_ISO" ]; then
        echo "ERROR: Failed to generate Windows 11 ISO via UUP dump."
        exit 1
      fi

      echo "==> [3/3] Archiving Windows 11 ISO to $OUTPUT_ISO..."
      mv "$GENERATED_ISO" "$OUTPUT_ISO"
      chmod 0644 "$OUTPUT_ISO"
      echo "==> Successfully created Windows 11 ISO: $OUTPUT_ISO"
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
    uupEnv
    buildIsoApp
    ;
}
