{
  createFlakeModule,
  declareNixosModule,
  ...
}:
createFlakeModule (
  declareNixosModule (
    {
      pkgs,
      config,
      ...
    }:
  {
    artifacts.store.rclone-backup = {
      prompts = {
        client_id.description = "Google Drive OAuth Client ID";
        client_secret.description = "Google Drive OAuth Client Secret";
        token.description = "Google Drive OAuth Token JSON";
      };
      generator = pkgs.writeShellScript "gen-rclone-config" ''
        export PATH="${pkgs.coreutils}/bin:$PATH"

        CLIENT_ID="$(tr -d '\r\n ' < "$prompts/client_id")"
        CLIENT_SECRET="$(tr -d '\r\n ' < "$prompts/client_secret")"

        printf '[gdrive]\ntype = drive\nscope = drive\nclient_id = %s\nclient_secret = %s\ntoken = ' \
          "$CLIENT_ID" "$CLIENT_SECRET" > "$out/config"
        cat "$prompts/token" >> "$out/config"
        printf '\n' >> "$out/config"
      '';
      files.config = {
        owner = "root";
        group = "root";
        mode = "0600";
      };
    };

    artifacts.store.restic-backup = {
      prompts.password.description = "Enter Restic repository password (or press Enter to auto-generate random 32-char password)";
      generator = pkgs.writeShellScript "gen-restic-password" ''
        PASS="$(tr -d '[:space:]' < "$prompts/password")"
        if [ -n "$PASS" ]; then
          echo "$PASS" > "$out/password"
        else
          ${pkgs.pwgen}/bin/pwgen -s 32 1 > "$out/password"
        fi
      '';
      files.password = {
        owner = "root";
        group = "root";
        mode = "0400";
      };
    };

    services.restic.backups.depot = {
      repository = "rclone:gdrive:backups/gandalf-depot";
      rcloneConfigFile = config.artifacts.store.rclone-backup.files.config.path;
      passwordFile = config.artifacts.store.restic-backup.files.password.path;
      paths = [ "/data/depot" ];
      initialize = true;
      extraOptions = [
        "rclone.program=${pkgs.rclone}/bin/rclone"
      ];
      timerConfig = {
        OnCalendar = "02:00";
        Persistent = true;
      };
      pruneOpts = [
        "--keep-daily 7"
        "--keep-weekly 4"
        "--keep-monthly 12"
      ];
    };

    systemd.services.restic-backups-depot.path = [ pkgs.rclone ];
  }
  )
)
