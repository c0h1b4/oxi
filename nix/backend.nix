{ crane, fenix, ... }:
{
  perSystem =
    {
      system,
      pkgs,
      lib,
      ...
    }:
    let
      toolchain = fenix.packages.${system}.stable.defaultToolchain;
      craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
      root = ../backend;
      src = lib.fileset.toSource {
        inherit root;
        fileset = lib.fileset.unions [
          (craneLib.fileset.commonCargoSources root)
          (root + "/migrations")
        ];
      };
      commonArgs = {
        inherit src;
        pname = "oxi-email-server";
        strictDeps = true;
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = [ pkgs.openssl ];
      };
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;
    in
    {
      packages.backend = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;
        }
      );
    };
}
