{ ... }:
let
  versions = {
    "26300.9457.pro.en-us" = {
      uupId = "ba6eafb7-6fd3-4fb9-8462-3d86b81835b3";
      edition = "professional";
      language = "en-us";
    };
    "26300.9457.pro.ja-jp" = {
      uupId = "ba6eafb7-6fd3-4fb9-8462-3d86b81835b3";
      edition = "professional";
      language = "ja-jp";
    };
  };

  resolveVersion =
    versionKey:
    let
      v = versions.${versionKey} or null;
    in
    if v != null then
      v
    else
      throw "Unknown Windows version key: '${versionKey}'. Supported versions: ${builtins.concatStringsSep ", " (builtins.attrNames versions)}";
in
{
  inherit versions resolveVersion;
}
