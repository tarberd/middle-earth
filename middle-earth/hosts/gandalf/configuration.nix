{
  mkFlakeModule,
  super,
  antigravity-nix,
  ...
}@inputs:
mkFlakeModule.withNixosModule { } (
  {
    pkgs,
    ...
  }:
  {
    system.stateVersion = "26.05";

    imports = [
      super.hardware-configuration
      super.storage
      super.network
      super.virtualization.kvm
      super.virtualization.container
      super.backup
    ];

    boot.loader.systemd-boot.enable = true;
    boot.loader.efi.canTouchEfiVariables = true;
    boot.kernelPackages = pkgs.linuxPackages_latest;
    boot.initrd.kernelModules = [ "xe" ];

    nix.settings.experimental-features = [
      "nix-command"
      "flakes"
    ];
    nix.settings.auto-optimise-store = true;
    nix.gc = {
      automatic = true;
      dates = "weekly";
      options = "--delete-older-than 14d";
    };

    time.timeZone = "Europe/Amsterdam";

    i18n = {
      defaultLocale = "en_US.UTF-8";
      extraLocales = [ "pt_BR.UTF-8/UTF-8" ];
      extraLocaleSettings = {
        LC_CTYPE = "pt_BR.UTF-8";
      };
    };

    console = {
      font = "Lat2-Terminus16";
      keyMap = "us";
    };

    middle-earth.roles = {
      admin = [ "wheel" ];
      desktop = [
        "audio"
        "video"
        "networkmanager"
        "lp"
        "scanner"
      ];
    };
    security.sudo.wheelNeedsPassword = false;

    services.printing = {
      enable = true;
      drivers = [ pkgs.epson-escpr2 ];
      browsed.enable = true;
    };

    services.avahi = {
      enable = true;
      nssmdns4 = true;
      reflector = true;
      allowInterfaces = [
        "eth0"
        "eth1"
        "virbr0"
      ];
    };

    hardware.sane = {
      enable = true;
      extraBackends = [
        pkgs.sane-airscan
        pkgs.epsonscan2
      ];
    };

    hardware.printers = {
      ensurePrinters = [
        {
          name = "EPSON_EP-8550";
          location = "Local Network";
          description = "Epson EcoTank EP-8550 / ET-8550";
          deviceUri = "ipp://EPSON5CA9BE.local:631/ipp/print";
          model = "everywhere";
        }
      ];
      ensureDefaultPrinter = "EPSON_EP-8550";
    };

    environment.etc."sane.d/airscan.conf".text = ''
      [devices]
      "EPSON EP-8550" = https://EPSON5CA9BE.local:443/eSCL/
    '';

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

    nixpkgs.config.allowUnfree = true;
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
      nixfmt
      cargo
      rustc
      rust-analyzer
      rustfmt
      polkit
      polkit_gnome
      antigravity-nix.packages.x86_64-linux.default # Base App
      antigravity-nix.packages.x86_64-linux.google-antigravity-ide # IDE
      antigravity-nix.packages.x86_64-linux.google-antigravity-cli # CLI
      claude-code
    ];

    nix.nixPath = [ "nixpkgs=${inputs.nixpkgs}" ];
  }
)
