{ superModule, ...} @ inputs:
{ pkgs, ... }:
{
  system.stateVersion = "26.05";

  imports = [
    superModule.hardware-configuration
    superModule.storage
    superModule.network
    superModule.kvm
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

  security.sudo.wheelNeedsPassword = false;

  services.printing.enable = true;

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
    openssh
    git
    ripgrep
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
  ];

  nix.nixPath = [ "nixpkgs=${inputs.nixpkgs}" ];
}
