# Changelog

All notable changes are recorded here. oxi uses Semantic Versioning and publishes
0.x releases as beta prereleases. See [the release process](docs/RELEASING.md).

## [Unreleased]

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

[Unreleased]: https://github.com/c0h1b4/oxi/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/c0h1b4/oxi/releases/tag/v0.1.0
