inputs: {
  imports = [
    (import ./backend.nix inputs)
    (import ./frontend.nix inputs)
    (import ./checks.nix inputs)
  ];

  perSystem =
    { pkgs, self', lib, ... }:
    let
      backend = self'.packages.backend;
      mkPackage =
        { basePath ? "" }:
        let
          frontend = self'.packages.frontend.override { inherit basePath; };
        in
        pkgs.runCommand "oxi-email-server"
          {
            nativeBuildInputs = [ pkgs.makeWrapper ];
            passthru.oxiBasePath = basePath;
          }
          ''
            mkdir -p $out/bin $out/share/oxi
            cp -r ${frontend} $out/share/oxi/static
            cp ${backend}/bin/oxi-email-server $out/bin/.oxi-email-server-wrapped
            makeWrapper $out/bin/.oxi-email-server-wrapped $out/bin/oxi-email-server \
              --set-default STATIC_DIR "$out/share/oxi/static" \
              --set-default ENVIRONMENT "production" \
              --set-default BASE_PATH ${lib.escapeShellArg basePath} \
              --set SSL_CERT_FILE "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          '';
    in
    {
      packages.default = lib.makeOverridable mkPackage { };
    };
}
