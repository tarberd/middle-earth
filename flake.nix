{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixvirt = {
      url = "https://flakehub.com/f/AshleyYakeley/NixVirt/*.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {self, nixpkgs, ...}@inputs: (
    let
      inherit (inputs.nixpkgs.lib) hasSuffix removeSuffix recursiveUpdate;

      # 1. Strict directory reader that throws on file/folder collisions
      readDirSafe = dir:
        let
          entries = builtins.readDir dir;
        in
          builtins.foldl' (acc: name:
            let
              type = entries.${name};
              isNix = type == "regular" && hasSuffix ".nix" name;
              cleanName = removeSuffix ".nix" name;
            in
              if type != "directory" && !isNix then
                acc # Ignore non-nix files like README.md
              else if acc ? ${cleanName} then
                throw "Name collision in ${toString dir}: Cannot map both '${name}' and an existing file/folder to the same attribute '${cleanName}'."
              else
                acc // {
                  ${cleanName} = {
                    inherit name type;
                    path = dir + "/${name}";
                  };
                }
          ) {} (builtins.attrNames entries);

      # 2. Build the un-evaluated path tree safely
      getNixTree = dir:
        let
          safeEntries = readDirSafe dir;

          processEntry = cleanName: info:
            if info.type == "directory" then
              getNixTree info.path
            else
              if info.name == "default.nix" then dir else info.path;
        in
          builtins.mapAttrs processEntry safeEntries;
    in
      let
        # Assuming this is called at the flake root, exclude flake.nix itself
        localModules = removeAttrs (getNixTree ./.) [ "flake" ];

        # 2. Custom evaluator that traverses, imports, and flattens
        evaluateAndFlatten = unEvaluatedNode: selfRef: superRef: args:
          if builtins.isAttrs unEvaluatedNode then
            let
              # Recursively evaluate all children in this directory
              evaluatedChildren = builtins.mapAttrs
                (
                  name: child: (
                    let
                      childSelf = if name == "default" then selfRef else selfRef.${name};
                      childSuper = if name == "default" then superRef else selfRef;
                    in
                      evaluateAndFlatten child childSelf childSuper args
                  )
                )
                unEvaluatedNode
              ;
            in
              # If a 'default' exists and evaluated to an attribute set, flatten it!
              if evaluatedChildren ? default && builtins.isAttrs evaluatedChildren.default then
                # Sibling modules (like 'foo') take precedence in case of naming collisions
                recursiveUpdate evaluatedChildren.default (removeAttrs evaluatedChildren [ "default" ])
              else
                evaluatedChildren
          else
            # Base case: we hit a file path, so import it and apply the moduleArgs
            (import unEvaluatedNode) (args // { selfModule = selfRef; superModule = superRef; });

        moduleArgs = inputs // { inherit localModules; } // evaluatedModules;
        evaluatedModules = evaluateAndFlatten localModules evaluatedModules null moduleArgs;
      in
        evaluatedModules
  );
}
