{
  mkFlakeModule,
  super,
  ...
}@inputs:
mkFlakeModule.withNixosModule { } (
  {
    lib,
    pkgs,
    ...
  }:
  {
    system.stateVersion = "26.05";

    imports = [
      super.disko-config
      super.hardware-configuration
    ];

    boot.loader.systemd-boot = {
      enable = true;
      configurationLimit = 10;
    };
    boot.loader.efi.canTouchEfiVariables = true;

    nix.settings = {
      experimental-features = [
        "nix-command"
        "flakes"
      ];
      auto-optimise-store = true;
    };
    nix.gc = {
      automatic = true;
      dates = "weekly";
      options = "--delete-older-than 14d";
    };

    time.timeZone = "America/Sao_Paulo";

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

    networking = {
      hostName = "smeagol";
      networkmanager.enable = true;
    };

    middle-earth.roles = {
      admin = [ "wheel" ];
      desktop = [
        "audio"
        "video"
        "networkmanager"
      ];
    };
    security.sudo.wheelNeedsPassword = false;

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
      # Replace with /mnt/etc/ssh/ssh_host_ed25519_key.pub generated during install
      publicHostKey = "ssh-ed25519 REPLACE_WITH_SMEAGOL_HOST_KEY";
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

    nixpkgs.config = {
      allowUnfree = true;
      allowInsecurePredicate = pkg: builtins.elem (lib.getName pkg) [ "broadcom-sta" ];
    };

    environment.systemPackages = with pkgs; [
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
      pciutils
      foot
      wmenu
      grim
      slurp
      wl-clipboard
      swaybg
      waybar
      pavucontrol
      polkit
      polkit_gnome
    ];

    nix.nixPath = [ "nixpkgs=${inputs.nixpkgs}" ];
  }
)
