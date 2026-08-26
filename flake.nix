{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixvirt = {
      url = "https://flakehub.com/f/AshleyYakeley/NixVirt/*.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, disko, nixvirt, ... } @ inputs: {
    nixosConfigurations.gandalf = inputs.nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      specialArgs = { inherit inputs; };
      modules = [
        disko.nixosModules.disko
        nixvirt.nixosModules.default
        ./middle-earth/hosts/gandalf/disko-config.nix
        ./middle-earth/hosts/gandalf/configuration.nix
        ./middle-earth/hosts/gandalf/hardware-configuration.nix
      ];
    };

    nixosConfigurations.sauron = inputs.nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      specialArgs = { inherit inputs; };
      modules = [
        disko.nixosModules.disko
        ./middle-earth/hosts/sauron/disko-config.nix
        ./middle-earth/hosts/sauron/configuration.nix
      ];
    };
  };
}
