{ bun2nix, ... }:
{
  perSystem =
    { system, ... }:
    let
      bunDeps = bun2nix.packages.${system}.default.fetchBunDeps {
        bunNix = ../frontend/.bun.nix;
      };
    in
    {
      packages.frontend = bun2nix.packages.${system}.default.mkDerivation {
        pname = "oxi-frontend";
        version = "0.1.0";
        src = ../frontend;
        inherit bunDeps;

        buildPhase = ''
          export NEXT_TELEMETRY_DISABLED=1
          bun run build
        '';

        installPhase = ''
          mkdir -p $out
          cp -r out/* $out/
        '';
      };
    };
}
