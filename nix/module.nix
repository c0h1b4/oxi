self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.oxi;
  basePath = if cfg.basePath == null then "" else cfg.basePath;
in
{
  options.services.oxi = {
    enable = lib.mkEnableOption "oxi webmail client";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default.override { inherit basePath; };
      defaultText = lib.literalExpression ''
        self.packages.''${pkgs.system}.default.override {
          basePath = if config.services.oxi.basePath == null then "" else config.services.oxi.basePath;
        }
      '';
      description = "The oxi package to use. Its oxiBasePath passthru must match basePath.";
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

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Runtime secret environment file, e.g. /run/secrets/oxi.env containing DATABASE_URL. Do not place secrets in the Nix store.";
    };

    databaseMaxConnections = lib.mkOption {
      type = lib.types.ints.positive;
      default = 10;
      description = "Maximum PostgreSQL connections per process.";
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
      description = ''
        Reverse proxy subpath (for example /oxi), without a trailing slash.
        null or "" serves at the root. The default package builds the static
        frontend for this prefix. Custom packages must expose matching
        passthru.oxiBasePath metadata.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = basePath == "" || builtins.match "/[A-Za-z0-9_-]+(/[A-Za-z0-9_-]+)*" basePath != null;
        message = "services.oxi.basePath must be empty or a slash-prefixed path without a trailing slash (for example /oxi).";
      }
      {
        assertion = (cfg.package.oxiBasePath or null) == basePath;
        message = "services.oxi.package must expose oxiBasePath matching services.oxi.basePath; build it with .override { basePath = ...; }. The frontend prefix is fixed at build time.";
      }
    ];

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
        DATABASE_MAX_CONNECTIONS = toString cfg.databaseMaxConnections;
        STATIC_DIR = "${cfg.package}/share/oxi/static";
        IMAP_HOST = cfg.imapHost;
        IMAP_PORT = toString cfg.imapPort;
        SMTP_HOST = cfg.smtpHost;
        SMTP_PORT = toString cfg.smtpPort;
        TLS_ENABLED = lib.boolToString cfg.tlsEnabled;
        SESSION_TIMEOUT_HOURS = toString cfg.sessionTimeoutHours;
        ENVIRONMENT = "production";
        RUST_LOG = cfg.rustLog;
        BASE_PATH = basePath;
      };

      serviceConfig = {
        Type = "simple";
        EnvironmentFile = lib.optional (cfg.environmentFile != null) cfg.environmentFile;
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
