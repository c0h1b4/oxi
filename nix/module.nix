self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.oxi;
in
{
  options.services.oxi = {
    enable = lib.mkEnableOption "oxi webmail client";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default;
      description = "The oxi package to use.";
    };

    host = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = "Address to bind the HTTP server to.";
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 3001;
      description = "Port for the HTTP server.";
    };

    dataDir = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/oxi";
      description = "Directory for SQLite database and Tantivy indexes.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "oxi";
      description = "User to run the service as.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "oxi";
      description = "Group to run the service as.";
    };

    imapHost = lib.mkOption {
      type = lib.types.str;
      description = "IMAP server hostname.";
    };

    imapPort = lib.mkOption {
      type = lib.types.port;
      default = 993;
      description = "IMAP server port.";
    };

    smtpHost = lib.mkOption {
      type = lib.types.str;
      description = "SMTP server hostname.";
    };

    smtpPort = lib.mkOption {
      type = lib.types.port;
      default = 587;
      description = "SMTP server port.";
    };

    tlsEnabled = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether TLS is enabled for mail connections.";
    };

    sessionTimeoutHours = lib.mkOption {
      type = lib.types.int;
      default = 24;
      description = "Session timeout in hours.";
    };

    rustLog = lib.mkOption {
      type = lib.types.str;
      default = "info";
      description = "Rust log level.";
    };

    basePath = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Optional base path prefix for reverse proxy subpath.";
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.${cfg.user} = {
      isSystemUser = true;
      group = cfg.group;
      home = cfg.dataDir;
      createHome = true;
    };
    users.groups.${cfg.group} = { };

    systemd.services.oxi = {
      description = "oxi webmail";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];

      environment = {
        HOST = cfg.host;
        PORT = toString cfg.port;
        DATA_DIR = cfg.dataDir;
        STATIC_DIR = "${cfg.package}/share/oxi/static";
        IMAP_HOST = cfg.imapHost;
        IMAP_PORT = toString cfg.imapPort;
        SMTP_HOST = cfg.smtpHost;
        SMTP_PORT = toString cfg.smtpPort;
        TLS_ENABLED = lib.boolToString cfg.tlsEnabled;
        SESSION_TIMEOUT_HOURS = toString cfg.sessionTimeoutHours;
        ENVIRONMENT = "production";
        RUST_LOG = cfg.rustLog;
      }
      // lib.optionalAttrs (cfg.basePath != null) {
        BASE_PATH = cfg.basePath;
      };

      serviceConfig = {
        Type = "simple";
        ExecStart = "${cfg.package}/bin/oxi-email-server";
        WorkingDirectory = cfg.dataDir;
        User = cfg.user;
        Group = cfg.group;
        Restart = "on-failure";
        RestartSec = 5;

        # Filesystem
        ReadWritePaths = [ cfg.dataDir ];
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;

        # Security hardening
        NoNewPrivileges = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
        CapabilityBoundingSet = "";
        SystemCallFilter = [ "@system-service" ];
        RestrictNamespaces = true;
        LockPersonality = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        MemoryDenyWriteExecute = true;
        PrivateDevices = true;
      };
    };
  };
}
