{
  description = "tarberd's configuration flake";

  inputs = {
    nixpkgs = {
      url = "github:nixos/nixpkgs?ref=nixos-unstable";
    };
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixos-artifacts = {
      url = "github:mrVanDalo/nixos-artifacts";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixos-artifacts-agenix = {
      url = "github:mrVanDalo/nixos-artifacts-agenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixvirt = {
      url = "https://flakehub.com/f/AshleyYakeley/NixVirt/*.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    antigravity-nix = {
      url = "github:jacopone/antigravity-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    terranix = {
      url = "github:terranix/terranix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-modules = {
      url = "github:tarberd/flake-modules";
    };
  };

  outputs = inputs@{ flake-modules, ... }:
    flake-modules.lib.evalFlake ./. inputs;
}
