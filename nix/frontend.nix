{ bun2nix, ... }:
{
  perSystem =
    { system, lib, ... }:
    let
      bunDeps = bun2nix.packages.${system}.default.fetchBunDeps {
        bunNix = ../frontend/.bun.nix;
      };
      mkFrontend =
        { basePath ? "" }:
        bun2nix.packages.${system}.default.mkDerivation {
          pname = "oxi-frontend";
          version = "0.1.0";
          src = ../frontend;
          inherit bunDeps;
          NEXT_PUBLIC_BASE_PATH = basePath;
          passthru.oxiBasePath = basePath;

          buildPhase = ''
            export NEXT_TELEMETRY_DISABLED=1
            bun run build
          '';

          installPhase = ''
            if [ ! -f out/index.html ]; then
              echo "Next static export output 'out/index.html' is missing" >&2
              exit 1
            fi
            mkdir -p $out
            cp -r out/* $out/
          '';
        };
    in
    {
      packages.frontend = lib.makeOverridable mkFrontend { };
    };
}
