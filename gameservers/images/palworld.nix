{
  createFlakeModule,
  nixpkgs,
  ...
}:
let
  system = "x86_64-linux";
  pkgs = nixpkgs.legacyPackages.${system};

  serviceFile = pkgs.writeText "palworld.service" ''
    [Unit]
    Description=Palworld Dedicated Server
    After=network.target network-online.target
    Wants=network-online.target

    [Service]
    Type=simple
    User=steam
    Group=steam
    Restart=on-failure
    RestartSec=10
    LimitNOFILE=100000
    ExecStartPre=/usr/local/bin/palworld-init
    ExecStart=/usr/local/bin/palworld-run
    WorkingDirectory=/opt/palworld

    [Install]
    WantedBy=multi-user.target
  '';

  initScript = pkgs.writeScript "palworld-init" ''
    #!/usr/bin/env bash
    set -euo pipefail

    echo "==> [palworld-init] Initializing persistent directories in /data..."
    mkdir -p /data/Config/LinuxServer /data/SaveGames

    echo "==> [palworld-init] Linking /opt/palworld/Pal/Saved to /data..."
    mkdir -p /opt/palworld/Pal
    if [ ! -L /opt/palworld/Pal/Saved ]; then
      if [ -d /opt/palworld/Pal/Saved ]; then
        cp -rn /opt/palworld/Pal/Saved/* /data/ 2>/dev/null || true
        rm -rf /opt/palworld/Pal/Saved
      fi
      ln -sfn /data /opt/palworld/Pal/Saved
    fi

    echo "==> [palworld-init] Ensuring Steam SDK library is linked..."
    mkdir -p /home/steam/.steam/sdk64
    if [ -f /home/steam/.steam/steamcmd/linux64/steamclient.so ]; then
      ln -sfn /home/steam/.steam/steamcmd/linux64/steamclient.so /home/steam/.steam/sdk64/steamclient.so
    fi

    if [ ! -f /data/Config/LinuxServer/PalWorldSettings.ini ] && [ -f /opt/palworld/DefaultPalWorldSettings.ini ]; then
      echo "==> [palworld-init] Generating default PalWorldSettings.ini in /data/Config/LinuxServer/..."
      cp /opt/palworld/DefaultPalWorldSettings.ini /data/Config/LinuxServer/PalWorldSettings.ini
    fi

    echo "==> [palworld-init] Container is immutable; ready to launch server."
  '';

  runScript = pkgs.writeScript "palworld-run" ''
    #!/usr/bin/env bash
    set -euo pipefail

    echo "==> [palworld-run] Starting PalServer..."
    exec /opt/palworld/PalServer.sh -useperfthreads -NoAsyncLoadingThread -UseMultithreadforDS "$@"
  '';

  provisionScript = pkgs.writeScript "provision-palworld.sh" ''
    #!/usr/bin/env bash
    set -euo pipefail

    echo "==> [1/6] Configuring Dutch mirrors and enabling multilib / parallel downloads..."
    sed -i '/\[multilib\]/,/Include/ s/^#//' /etc/pacman.conf
    sed -i 's/^#ParallelDownloads = 5/ParallelDownloads = 5/' /etc/pacman.conf

    FETCHED=0
    if command -v curl >/dev/null 2>&1; then
      if curl -sSfL "https://archlinux.org/mirrorlist/?country=NL&protocol=https&ip_version=4&use_mirror_status=on" | sed 's/^#Server/Server/' > /etc/pacman.d/mirrorlist; then
        FETCHED=1
      fi
    elif command -v wget >/dev/null 2>&1; then
      if wget -qO- "https://archlinux.org/mirrorlist/?country=NL&protocol=https&ip_version=4&use_mirror_status=on" | sed 's/^#Server/Server/' > /etc/pacman.d/mirrorlist; then
        FETCHED=1
      fi
    fi

    if [ "$FETCHED" -eq 0 ] || ! grep -q '^Server' /etc/pacman.d/mirrorlist 2>/dev/null; then
      cat << 'EOF' > /etc/pacman.d/mirrorlist
Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch
Server = https://mirror.rackspace.com/archlinux/$repo/os/$arch
EOF
      pacman -Sy --noconfirm curl
      curl -sSfL "https://archlinux.org/mirrorlist/?country=NL&protocol=https&ip_version=4&use_mirror_status=on" | sed 's/^#Server/Server/' > /etc/pacman.d/mirrorlist
    fi

    echo "==> [2/6] Updating system and installing dependencies..."
    pacman -Syu --noconfirm
    pacman -S --noconfirm --needed \
      base-devel \
      git \
      glibc \
      gcc-libs \
      lib32-glibc \
      lib32-gcc-libs \
      ca-certificates \
      curl \
      wget \
      tar \
      procps-ng \
      xdg-user-dirs

    echo "==> [3/6] Setting up steam user (UID 1000)..."
    if id -u arch >/dev/null 2>&1; then
      pkill -u arch || true
      usermod --login steam --home /home/steam --move-home arch
      groupmod --new-name steam arch
    elif ! id -u steam >/dev/null 2>&1; then
      useradd -m -u 1000 -s /bin/bash steam
    fi

    echo "==> [4/6] Building and installing steamcmd from AUR..."
    su - steam -c "
      rm -rf /tmp/steamcmd-aur
      git clone https://aur.archlinux.org/steamcmd.git /tmp/steamcmd-aur
      cd /tmp/steamcmd-aur
      makepkg --noconfirm --skipchecksums --skippgpcheck
    "
    pacman -U --noconfirm /tmp/steamcmd-aur/steamcmd-*.pkg.tar.zst
    rm -rf /tmp/steamcmd-aur

    echo "==> [5/6] Bootstrapping SteamCMD and pre-downloading Palworld server..."
    mkdir -p /opt/palworld /home/steam/.steam/sdk64
    chown -R steam:steam /opt/palworld /home/steam

    # Initial steamcmd run to populate runtime files in ~/.steam/steamcmd
    su - steam -c "steamcmd +quit" || true

    # Link 64-bit Steamworks SDK library for PalServer
    if [ -f /home/steam/.steam/steamcmd/linux64/steamclient.so ]; then
      ln -sfn /home/steam/.steam/steamcmd/linux64/steamclient.so /home/steam/.steam/sdk64/steamclient.so
    fi

    # Pre-download Palworld Dedicated Server with retry loop (Steam API often returns transient Missing configuration on 1st try)
    echo "==> Downloading Palworld server files via SteamCMD..."
    DOWNLOADED=0
    for i in $(seq 1 5); do
      echo "--> SteamCMD download attempt $i/5..."
      if su - steam -c "steamcmd +@sSteamCmdForcePlatformType linux +force_install_dir /opt/palworld +login anonymous +app_update 2394010 validate +quit"; then
        if [ -f /opt/palworld/PalServer.sh ]; then
          DOWNLOADED=1
          break
        fi
      fi
      echo "--> Attempt $i did not complete, waiting 5 seconds before retrying..."
      sleep 5
    done

    if [ "$DOWNLOADED" -ne 1 ]; then
      echo "ERROR: PalServer.sh was not found after 5 SteamCMD download attempts!"
      exit 1
    fi
    chmod +x /opt/palworld/PalServer.sh

    echo "==> [6/6] Finalizing golden image configuration..."
    pacman -Scc --noconfirm
    systemctl enable systemd-networkd
    systemctl enable palworld.service
    rm -f /tmp/provision.sh
    echo "==> Provisioning complete!"
  '';

  buildApp = pkgs.writeShellApplication {
    name = "build-palworld-image";
    runtimeInputs = [
      pkgs.incus
      pkgs.coreutils
      pkgs.iputils
    ];
    text = ''
      TAG="''${1:-latest}"
      BUILDER="palworld-builder-$$"
      ARCHIVE_DIR="/data/depot/games/images"

      cleanup() {
        if incus info "$BUILDER" >/dev/null 2>&1; then
          echo "==> Cleaning up builder container $BUILDER..."
          incus stop "$BUILDER" --force >/dev/null 2>&1 || true
          incus delete "$BUILDER" >/dev/null 2>&1 || true
        fi
      }
      trap cleanup EXIT

      echo "==> [1/5] Launching fresh builder container from images:archlinux/cloud..."
      incus launch images:archlinux/cloud "$BUILDER"

      echo "==> [2/5] Waiting for network connectivity in builder container..."
      CONNECTED=0
      for _ in $(seq 1 45); do
        if incus exec "$BUILDER" -- ping -c 1 -W 1 8.8.8.8 >/dev/null 2>&1; then
          CONNECTED=1
          break
        fi
        sleep 1
      done

      if [ "$CONNECTED" -ne 1 ]; then
        echo "Error: Network did not become ready in container $BUILDER"
        exit 1
      fi

      echo "==> [3/5] Pushing provisioning files..."
      incus exec "$BUILDER" -- mkdir -p /usr/local/bin /etc/systemd/system /tmp
      incus file push "${provisionScript}" "$BUILDER/tmp/provision.sh" --mode=0755
      incus file push "${serviceFile}" "$BUILDER/etc/systemd/system/palworld.service" --mode=0644
      incus file push "${initScript}" "$BUILDER/usr/local/bin/palworld-init" --mode=0755
      incus file push "${runScript}" "$BUILDER/usr/local/bin/palworld-run" --mode=0755

      echo "==> [4/5] Executing AUR build and Palworld download inside container..."
      incus exec "$BUILDER" -- /tmp/provision.sh

      echo "==> [5/5] Publishing golden image as palworld/$TAG..."
      incus stop "$BUILDER"
      incus publish "$BUILDER" --alias "palworld/$TAG" --alias "palworld/latest" --reuse

      echo "==> Archiving image to $ARCHIVE_DIR/palworld-$TAG..."
      mkdir -p "$ARCHIVE_DIR"
      incus image export "palworld/$TAG" "$ARCHIVE_DIR/palworld-$TAG"

      echo "==> Successfully created, published, and archived palworld/$TAG!"
    '';
  };
in
createFlakeModule {
  inherit serviceFile initScript runScript provisionScript buildApp;
}
