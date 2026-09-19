{
  superModule,
  antigravity-nix,
  declareNixosModule,
  ...
} @ inputs:
declareNixosModule (
  {
    pkgs,
    ...
  }:
  {
    system.stateVersion = "26.05";

    imports = [
      superModule.hardware-configuration
      superModule.storage
      superModule.network
      superModule.virtualization.kvm
      superModule.virtualization.container
      superModule.backup
    ];

    boot.loader.systemd-boot.enable = true;
    boot.loader.efi.canTouchEfiVariables = true;
    boot.kernelPackages = pkgs.linuxPackages_latest;
    boot.initrd.kernelModules = [ "xe" ];

    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    nix.settings.auto-optimise-store = true;
    nix.gc = {
      automatic = true;
      dates = "weekly";
      options = "--delete-older-than 14d";
    };

    time.timeZone = "Europe/Amsterdam";
    i18n.defaultLocale = "en_US.UTF-8";

    console = {
      font = "Lat2-Terminus16";
      keyMap = "us";
    };

    middle-earth.roles = {
      admin = [ "wheel" ];
      desktop = [ "audio" "video" "networkmanager" ];
    };
    security.sudo.wheelNeedsPassword = false;

    services.printing.enable = true;

    services.openssh = {
      enable = true;
      settings = {
        PasswordAuthentication = false;
        KbdInteractiveAuthentication = false;
        PermitRootLogin = "prohibit-password";
      };
    };

    users.users.root.openssh.authorizedKeys.keys = [
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB6a46GEO27tNA42ehDQkZClA4oNWypBDiOyc86OkNWO bernardo.mferrari@gmail.com"
    ];
    users.users.tarberd.openssh.authorizedKeys.keys = [
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB6a46GEO27tNA42ehDQkZClA4oNWypBDiOyc86OkNWO bernardo.mferrari@gmail.com"
    ];

    artifacts.default.backend = "agenix";
    artifacts.config.agenix = {
      flakeStoreDir = ../../../secrets;
      publicHostKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDWEK3UDyNqU4vK8d/C8HQzclf7AGkjEW533k5RpV9cJ root@gandalf";
      publicUserKeys = [
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB6a46GEO27tNA42ehDQkZClA4oNWypBDiOyc86OkNWO bernardo.mferrari@gmail.com"
      ];
    };

    services.pipewire = {
      enable = true;
      alsa.enable = true;
      alsa.support32Bit = true;
      pulse.enable = true;
    };

    programs.firefox.enable = true;
    programs.sway.enable = true;
    programs.zsh.enable = true;

    environment.systemPackages = with pkgs; [
      rclone
      openssh
      git
      ripgrep
      jq
      bat
      tree
      vim
      neovim
      wget
      curl
      tcpdump
      pciutils
      vulkan-tools
      code2prompt
      wl-clipboard
      swaybg
      waybar
      pavucontrol
      python3
      pipx
      nixd
      polkit
      polkit_gnome
      antigravity-nix.packages.x86_64-linux.default # Base App
      antigravity-nix.packages.x86_64-linux.google-antigravity-ide # IDE
      antigravity-nix.packages.x86_64-linux.google-antigravity-cli # CLI
    ];

    nix.nixPath = [ "nixpkgs=${inputs.nixpkgs}" ];
  }
)
