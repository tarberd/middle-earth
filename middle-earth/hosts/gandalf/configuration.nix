{ superModule, ...} @ inputs:
{ pkgs, ... }:
{
  imports = [
    superModule.disko-config
    superModule.hardware-configuration
    superModule.kvm
    superModule.network
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
    tree
    polkit
    polkit_gnome
  ];

  nix.nixPath = [ "nixpkgs=${inputs.nixpkgs}" ];

  system.stateVersion = "26.05";
}
