{
  createFlakeModule,
  nixpkgs,
  ...
}:
let
  inherit (nixpkgs) lib;
in
createFlakeModule {
  parseDotmanProfile = profilePath:
    let
      entries = builtins.readDir profilePath;
      dirs = builtins.filter (name: entries.${name} == "directory") (builtins.attrNames entries);

      processDir = dirName:
        let
          dirPath = profilePath + "/${dirName}";
          tomlPath = dirPath + "/dotman.toml";
        in if builtins.pathExists tomlPath then
          let
            toml = fromTOML (builtins.readFile tomlPath);
            dotman = toml.dotman or {};

            rawConfigPath = dotman.config_path or "$HOME";
            baseTarget =
              if rawConfigPath == "$HOME" then ""
              else lib.removePrefix "/" (lib.removePrefix "$HOME/" rawConfigPath);

            fileMapList = dotman.file_map or [];
            fileMap = builtins.listToAttrs (map (m: lib.nameValuePair m.file m.link) fileMapList);

            compEntries = builtins.readDir dirPath;
            compFiles = builtins.filter (n: n != "dotman.toml") (builtins.attrNames compEntries);

            links = builtins.listToAttrs (map (fileName:
              let
                targetFileName = fileMap.${fileName} or fileName;
                finalTargetPath = if baseTarget == "" then targetFileName else "${baseTarget}/${targetFileName}";
              in lib.nameValuePair finalTargetPath {
                source = dirPath + "/${fileName}";
              }
            ) compFiles);
          in links
        else {};
    in
      builtins.foldl' (acc: dir: acc // (processDir dir)) {} dirs;
}
