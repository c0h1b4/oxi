# Nix packaging and NixOS service

The flake exposes `packages.default` (the server with its static frontend),
`packages.backend`, `packages.frontend`, and `nixosModules.default`.

## Sandboxed builds

The Nix frontend uses Inter and JetBrains Mono from nixpkgs through
`next/font/local`. A Nix-only patch preserves the CSS variables and font weights
without fetching Google Fonts during compilation or changing development builds.
Both root and subpath asset checks require self-hosted fonts and reject Google
Fonts references in generated CSS.

The backend source fileset includes `tests/fixtures` as well as Cargo sources
and migrations. The authentication middleware tests embed a JSON fixture with
`include_str!`, so omitting it breaks the test build. Rust tests remain enabled.

## Root and subpath deployments

```nix
{
  imports = [ inputs.oxi.nixosModules.default ];

  services.oxi = {
    enable = true;
    imapHost = "mail.example.com";
    smtpHost = "mail.example.com";
    basePath = "/oxi";
  };
}
```

The default `basePath = null` and `basePath = ""` both serve at the root.
For a subpath, use a leading slash and no trailing slash, such as `/oxi` or
`/apps/oxi`. Path segments may contain letters, digits, `_`, and `-`.
The reverse proxy must preserve this prefix when forwarding requests.

The module builds the static frontend with `NEXT_PUBLIC_BASE_PATH` set to the
same prefix as the server's runtime `BASE_PATH`. Next.js embeds this prefix in
its assets and client code. Changing the runtime environment alone cannot
change an already-built frontend.

Both the combined package and the frontend are overridable:

```nix
let
  oxi = inputs.oxi.packages.${pkgs.system}.default.override {
    basePath = "/oxi";
  };
  frontend = inputs.oxi.packages.${pkgs.system}.frontend.override {
    basePath = "/oxi";
  };
in {
  services.oxi.package = oxi;
  services.oxi.basePath = "/oxi";
}
```

The combined package wrapper also defaults `BASE_PATH` to its build prefix for
standalone use. Do not override it to a different prefix.

Custom packages must expose `passthru.oxiBasePath` as `""` for root or the exact
subpath used when building the frontend. The module asserts that this metadata
matches `services.oxi.basePath`. Packages without this metadata are rejected
even for root deployments. Packages with a mismatched prefix also fail NixOS
configuration evaluation. Do not add metadata to
an incompatible prebuilt frontend just to bypass the assertion.

The Nix frontend build uses Bun for offline dependency installation and Node.js
24 to run Next.js and its page-data workers. This avoids a CommonJS loader
failure in the pinned Bun runtime during static export. Node is a build-time
dependency only; the deployed server still serves static assets from Rust.

## Checks

```console
nix build .#checks.x86_64-linux.module-base-path
nix build .#checks.x86_64-linux.frontend-root-assets
nix build .#checks.x86_64-linux.frontend-subpath-assets
# Or evaluate/build all checks for the current system:
nix flake check
```

`module-base-path` checks root, empty-root, nested subpath, and matching custom
packages, plus rejection of mismatched and metadata-free packages and trailing
slashes. The two frontend checks build real static exports and verify that
Next.js asset URLs have the expected prefix and refer to existing files.
They do not require an IMAP or SMTP server. The asset checker can also run on
an existing static export:

```console
python3 nix/check-static-assets.py frontend/out /oxi
```
