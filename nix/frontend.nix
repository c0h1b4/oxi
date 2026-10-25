{ bun2nix, ... }:
{
  perSystem =
    {
      system,
      pkgs,
      lib,
      ...
    }:
    let
      bunDeps = bun2nix.packages.${system}.default.fetchBunDeps {
        bunNix = ../frontend/.bun.nix;
      };
      mkFrontend =
        {
          basePath ? "",
        }:
        bun2nix.packages.${system}.default.mkDerivation {
          pname = "oxi-frontend";
          version = "0.1.0";
          src = ../frontend;
          inherit bunDeps;
          patches = [ ./local-fonts.patch ];
          # next/font/google fetches during compilation, which cannot work in
          # the Nix sandbox. Keep development builds unchanged and use fonts
          # supplied by nixpkgs for the packaged frontend.
          postPatch = ''
            mkdir -p src/app/fonts
            cp ${pkgs.inter}/share/fonts/truetype/InterVariable.ttf src/app/fonts/
            cp ${pkgs.jetbrains-mono}/share/fonts/truetype/JetBrainsMono-*.ttf src/app/fonts/
            if grep -R 'next/font/google' src; then
              echo "Oxi must not download Google Fonts in the Nix sandbox" >&2
              exit 1
            fi
          '';
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
            if ! find "$out/_next/static/media" -type f -name '*.ttf' | grep -q .; then
              echo "Oxi static export is missing its self-hosted fonts" >&2
              exit 1
            fi
          '';
        };
    in
    {
      packages.frontend = lib.makeOverridable mkFrontend { };
    };
}
