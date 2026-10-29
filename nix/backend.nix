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
          (root + "/postgres-migrations")
          # include_str! in the auth middleware tests needs this JSON fixture;
          # commonCargoSources only selects Cargo-related source files.
          (root + "/tests/fixtures")
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
