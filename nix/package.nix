inputs: {
  imports = [
    (import ./backend.nix inputs)
    (import ./frontend.nix inputs)
  ];

  perSystem =
    { pkgs, self', ... }:
    let
      backend = self'.packages.backend;
      frontend = self'.packages.frontend;
    in
    {
      packages.default =
        pkgs.runCommand "oxi-email-server"
          {
            nativeBuildInputs = [ pkgs.makeWrapper ];
          }
          ''
            mkdir -p $out/bin $out/share/oxi
            cp -r ${frontend} $out/share/oxi/static
            cp ${backend}/bin/oxi-email-server $out/bin/.oxi-email-server-wrapped
            makeWrapper $out/bin/.oxi-email-server-wrapped $out/bin/oxi-email-server \
              --set-default STATIC_DIR "$out/share/oxi/static" \
              --set-default ENVIRONMENT "production" \
              --set SSL_CERT_FILE "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          '';
    };
}
