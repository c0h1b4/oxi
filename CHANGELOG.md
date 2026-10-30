# Changelog

All notable changes are recorded here. oxi uses Semantic Versioning and labels
0.x releases as beta software. See [the release process](docs/RELEASING.md).

## [Unreleased]

## [0.1.3] - 2026-10-30

### Changed

- Split CI into reusable workflows behind one required `CI Gate`, with parallel
  Rust/frontend checks and cached Nix flake evaluation.
- Move container builds and GHCR/Docker Hub publication into a separate CD gate
  that releases the exact commit validated by main CI.
- Add a manual full Nix build workflow outside the fast required CI budget.
- Validate explicit pre-push boundaries for manual release recovery, including
  version bumps earlier in multi-commit pushes.
- Keep README deployment examples pinned to the published release.
- Release version bumps on `main` automatically after CI passes, with generated
  GitHub release notes and versioned Docker images.
- Require a version bump per main PR, with a `no-release` exception for maintenance.
- Add a patch-by-default version helper with minor, major, and exact-version support.
- Make Docker `latest` track numbered releases and `edge` track `main`; add
  floating minor tags and major tags starting at 1.0.
- Read the Nix frontend version from its package manifest.

## [0.1.2] - 2026-10-27

### Fixed

- Restore Enter/newline handling in the email reply editor.
- Align ProseMirror dependencies to prevent duplicate editor modules, with
  regression tests for keyboard handling and dependency resolution.

## [0.1.1] - 2026-10-26

### Fixed

- Recover missing older message headers after interrupted backfills and improve
  mailbox update recovery, including stale synchronization races.
- Invalidate the Sent cache after successful IMAP appends.
- Correct search-index removal and signature formatting.
- Improve sender privacy and reply handling.
- Fix Nix builds by bundling fonts for offline builds, including backend test
  fixtures, and running Next.js static exports with Node.js.

### Changed

- Update frontend dependencies to current stable releases.

## [0.1.0] - 2026-10-23

Initial versioned beta release.

### Included

- Self-hosted webmail backed by an existing IMAP/SMTP server.
- Message reading, rich-text composing, replies, forwarding, attachments, folders,
  and mailbox actions.
- Contacts, calendar views, conversation threads, and local full-text search.
- Real-time mailbox updates, per-user SQLite caching, and Tantivy search indexes.
- IMAP credential authentication, session expiry, CSRF protection, rate limiting,
  and input validation.
- A single Docker image serving both the React/Next.js frontend and Rust backend.
- Nix/NixOS deployment support and reverse-proxy base-path support.

### Release infrastructure

- Versioned Docker tags, with CI checking package and tag versions before publishing.
- Release notes, a changelog, and a documented release/upgrade checklist.

### Beta notice

- Breaking changes may occur before 1.0. Back up persistent data before upgrading.
- oxi is a client, not a mail server. A working IMAP/SMTP account is required.
- Docker images and source archives are provided; standalone release binaries are
  not currently published.

[Unreleased]: https://github.com/c0h1b4/oxi/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/c0h1b4/oxi/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/c0h1b4/oxi/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/c0h1b4/oxi/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/c0h1b4/oxi/releases/tag/v0.1.0
