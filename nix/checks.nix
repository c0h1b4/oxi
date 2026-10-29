{ nixpkgs, self, ... }:
{
  perSystem =
    { pkgs, self', lib, ... }:
    let
      evaluate =
        settings:
        (nixpkgs.lib.nixosSystem {
          system = pkgs.system;
          modules = [
            (import ./module.nix self)
            {
              boot.isContainer = true;
              fileSystems."/" = {
                device = "none";
                fsType = "tmpfs";
              };
              system.stateVersion = "25.11";
              services.oxi = {
                enable = true;
                imapHost = "mail.example.com";
                smtpHost = "mail.example.com";
              } // settings;
            }
          ];
        }).config;
      accepts = config: builtins.all (item: item.assertion) config.assertions;
      root = evaluate { };
      postgres = evaluate {
        environmentFile = "/run/secrets/oxi.env";
        databaseMaxConnections = 6;
      };
      empty = evaluate { basePath = ""; };
      subpath = evaluate { basePath = "/oxi"; };
      nested = evaluate { basePath = "/apps/oxi"; };
      custom = evaluate {
        basePath = "/oxi";
        package = self'.packages.default.override { basePath = "/oxi"; };
      };
      mismatch = evaluate {
        basePath = "/oxi";
        package = self'.packages.default;
      };
      rootMismatch = evaluate {
        package = self'.packages.default.override { basePath = "/oxi"; };
      };
      unknown = evaluate {
        package = pkgs.runCommand "oxi-without-prefix-metadata" { } "mkdir -p $out";
      };
      trailingSlash = evaluate { basePath = "/oxi/"; };
      rejected = config: !(builtins.tryEval (assert accepts config; true)).success;
      assetCheck =
        name: basePath:
        pkgs.runCommand name { nativeBuildInputs = [ pkgs.python3 ]; } ''
          python ${./check-static-assets.py} \
            ${self'.packages.frontend.override { inherit basePath; }} \
            ${lib.escapeShellArg basePath}
          touch $out
        '';
    in
    {
      checks = {
        module-base-path =
          assert accepts root && accepts empty && accepts subpath && accepts nested && accepts custom;
          assert postgres.systemd.services.oxi.serviceConfig.EnvironmentFile == [ "/run/secrets/oxi.env" ];
          assert postgres.systemd.services.oxi.environment.DATABASE_MAX_CONNECTIONS == "6";
          assert !(postgres.systemd.services.oxi.environment ? DATABASE_URL);
          assert root.services.oxi.package.oxiBasePath == "";
          assert empty.services.oxi.package.oxiBasePath == "";
          assert subpath.services.oxi.package.oxiBasePath == "/oxi";
          assert nested.services.oxi.package.oxiBasePath == "/apps/oxi";
          assert root.systemd.services.oxi.environment.BASE_PATH == "";
          assert subpath.systemd.services.oxi.environment.BASE_PATH == "/oxi";
          assert rejected mismatch && rejected rootMismatch && rejected unknown && rejected trailingSlash;
          pkgs.runCommand "oxi-module-base-path" { } "touch $out";
        frontend-root-assets = assetCheck "oxi-frontend-root-assets" "";
        frontend-subpath-assets = assetCheck "oxi-frontend-subpath-assets" "/oxi";
      };
    };
}
