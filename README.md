<div align="center">

<img src="assets/Gemini_Generated_Image_de71m3de71m3de71.png" alt="oxi logo" width="180" />

# oxi

**Modern, fast, and secure open-source webmail client built with React + Rust.**

A feature-complete replacement for Roundcube that connects to any existing IMAP/SMTP mail server.\
Not a mail platform — just a clean, modern webmail UI.

[![CI](https://github.com/c0h1b4/oxi/actions/workflows/ci.yml/badge.svg)](https://github.com/c0h1b4/oxi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/c0h1b4/oxi)](https://github.com/c0h1b4/oxi/stargazers)
[![Last commit](https://img.shields.io/github/last-commit/c0h1b4/oxi)](https://github.com/c0h1b4/oxi/commits/main)
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](https://ghcr.io/c0h1b4/oxi)

</div>

---

<!-- Add a screenshot here: ![oxi screenshot](assets/screenshot.png) -->

## Features

- **Mail** — Read, compose, reply, forward, and manage emails with a rich text editor (Tiptap)
- **Contacts** — Browse and manage your address book
- **Calendar** — iCal event parsing and calendar view
- **Full-Text Search** — Instant local search powered by Tantivy
- **Real-Time Updates** — WebSocket-driven live mailbox sync
- **Security** — IMAP credential auth, session tokens with configurable timeout, rate limiting
- **Fast** — Rust backend with per-user SQLite caching, static frontend served from the same binary
- **Self-Hosted** — Single Docker image, connects to any IMAP/SMTP server

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Frontend** | Next.js 16, React 19, Tailwind CSS 4, Shadcn/ui, Zustand, TanStack Query |
| **Backend** | Rust, Axum, Tokio, async-imap, Lettre (SMTP) |
| **Search** | Tantivy (Rust-native full-text search) |
| **Database** | SQLite per user (default) or shared PostgreSQL via async SQLx |
| **Editor** | Tiptap (rich text compose) |
| **Deploy** | Docker (GHCR), single binary serves frontend + API |

## Releases

**Current release: [v0.1.2 (Beta)](https://github.com/c0h1b4/oxi/releases/tag/v0.1.2).**
oxi now uses versioned releases. The 0.x series is beta software; breaking changes
may occur before 1.0. Back up persistent data before upgrading.

Use a versioned Docker tag for reproducible deployments. The `latest` image tracks
`main` and can include unreleased changes. See the [changelog](CHANGELOG.md) and
[release process](docs/RELEASING.md).

## Quick Start

### Docker (recommended)

```bash
docker run -d \
  --name oxi \
  -p 3001:3001 \
  -e IMAP_HOST=mail.example.com \
  -e SMTP_HOST=mail.example.com \
  -v oxi-data:/data \
  ghcr.io/c0h1b4/oxi:v0.1.2
```

Or with `docker-compose.yml`:

```yaml
services:
  app:
    image: ghcr.io/c0h1b4/oxi:v0.1.2
    ports:
      - "3001:3001"
    environment:
      - IMAP_HOST=mail.example.com
      - SMTP_HOST=mail.example.com
    volumes:
      - oxi-data:/data

volumes:
  oxi-data:
```

Then open [http://localhost:3001](http://localhost:3001) and log in with your email credentials.

### Local Development

```bash
# Frontend
cd frontend && bun install && bun run build

# Backend
cd backend && cargo build --release

# Run (serves frontend + API on port 3001)
STATIC_DIR=../frontend/out IMAP_HOST=mail.example.com SMTP_HOST=mail.example.com \
  ./target/release/oxi-email-server
```

### Separate frontend and backend origins

For local hot reload, start the backend with `ENVIRONMENT=development`,
`SERVE_STATIC=false`, and `CORS_ORIGIN=http://localhost:3000`. Start the frontend
with `NEXT_PUBLIC_API_URL=http://localhost:3001/api bun run dev`.
`NEXT_PUBLIC_API_URL` is the complete API root, including `/api` and any backend
path prefix. It is a build-time value for static exports. API calls, attachment
previews/downloads, inline draft images, contact exports, and WebSockets all use
this root. Cross-origin fetches include session cookies.

Use the same hostname and scheme on both origins (different localhost ports are
fine). Development cookies are SameSite=Lax, so unrelated cross-site hosts are
not supported; production uses SameSite=Strict and should serve the frontend and
API from the same origin. Cookie/browser third-party policies still apply.

For a same-origin subpath deployment, build the frontend with
`NEXT_PUBLIC_BASE_PATH=/oxi` and run the backend with `BASE_PATH=/oxi`.
An unset or empty `BASE_PATH` means the root path `/`. See [Nix deployment](nix/README.md)
for packages that build the matching prefix automatically.

## Configuration

All configuration is via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `0.0.0.0` | HTTP server bind address |
| `PORT` | `3001` | HTTP server port |
| `IMAP_HOST` | — | IMAP server hostname (**required**) |
| `IMAP_PORT` | `993` | IMAP server port |
| `SMTP_HOST` | — | SMTP server hostname (**required**) |
| `SMTP_PORT` | `587` | SMTP server port |
| `TLS_ENABLED` | `true` | Enable TLS for mail connections |
| `DATA_DIR` | `/data` | Persistent data storage directory (still required with PostgreSQL) |
| `DATABASE_URL` | unset | Optional PostgreSQL URL; unset keeps existing per-user SQLite files |
| `DATABASE_MAX_CONNECTIONS` | `10` | Maximum PostgreSQL pool connections per Oxi process |
| `SESSION_TIMEOUT_HOURS` | `24` | Session expiry in hours |
| `STATIC_DIR` | `./static` | Frontend static files directory |
| `ENVIRONMENT` | `development` | Runtime environment |
| `BASE_PATH` | — | Base path prefix for reverse proxy (e.g. `/oxi`) |
| `RUST_LOG` | `info` | Log level filter |

## PostgreSQL (optional)

Set `DATABASE_URL=postgres://user:password@host:5432/oxi` to use PostgreSQL for
all relational data. Without it, Oxi continues using existing SQLite files.
There is no automatic fallback if a configured PostgreSQL server is unavailable.
Migrations run before HTTP startup, using a migration lock. Use a dedicated database
and role that can create/alter its schema; back it up with standard PostgreSQL tools.
The default pool limit is 10 connections per process (plus one temporary migration
connection at startup); set `DATABASE_MAX_CONNECTIONS` to match your server budget.

For production, use certificate-verified TLS, for example
`?sslmode=verify-full&sslrootcert=/run/secrets/postgres-ca.crt`. Mount a private CA
when needed. Supply the URL through your deployment's secret mechanism, not source
control. URL-encode reserved characters in credentials.

For local Compose testing, set `POSTGRES_PASSWORD` in your environment and run:

```sh
docker compose -f docker-compose.yml -f docker-compose.postgres.yml up --build
```

The example uses a private Compose network without PostgreSQL TLS. Production
connections should use TLS. See `deploy/kubernetes-postgres.yaml` for a
single-replica Kubernetes example; build and publish the new image first, create
the `oxi-database` Secret separately, and replace the example mail hosts/image.

**PostgreSQL support does not make Oxi stateless or horizontally scalable.**
Keep one replica and persistent `DATA_DIR` storage for Tantivy indexes and draft
attachment bytes. Use a Recreate rollout to avoid concurrent writers on that
volume. Body-search indexes are not automatically rebuilt from already-cached
message bodies if the volume is lost. Sessions and live mail credentials remain
in memory, so pod restarts require login. Cross-pod sessions, event delivery, and
sync ownership are not supported. Back up draft files as well as the database.

`/api/health` is process liveness; `/api/ready` checks database availability after
initialization. Prefix these paths with `BASE_PATH` when deploying under a subpath;
the frontend must also be built with the matching prefix.

### Switching an existing installation

Selecting PostgreSQL starts with a new relational store; it does **not** import
SQLite data or delete the old files. IMAP cache data can be fetched again, but
contacts, identities, drafts, tags, settings, and locally created calendar records
are application-owned data. There is no built-in lossless importer in this release.
Do not switch a populated deployment without a data-transfer/backup plan. Preserve
the original SQLite files and attachment volume. The existing email-only account
hash remains unchanged; account-key redesign is outside this feature.

### Database tests

`cargo test` runs SQLite tests without requiring PostgreSQL. To run the same
repository suites against an isolated PostgreSQL test database:

```sh
OXI_TEST_DATABASE_URL=postgres://localhost/oxi_test cargo test
```

Use a dedicated disposable database: tests create isolated users and exercise
schema migrations. CI runs both modes.

## Architecture

```mermaid
graph LR
    Browser["Browser\n(Next.js)"] -->|HTTP / WS| Backend["Rust Backend\n(Axum)"]
    Backend -->|IMAP| IMAP["IMAP Server"]
    Backend -->|SMTP| SMTP["SMTP Server"]
    Backend --- Database["SQLite or PostgreSQL\n(cache + application data)"]
    Backend --- Tantivy["Tantivy\n(search)"]
```

## Development

```bash
# Run frontend dev server (hot reload)
cd frontend && bun run dev

# Run backend
cd backend && cargo run

# Run tests
cd frontend && bunx vitest run
cd backend && cargo test

# Lint
cd frontend && bun run lint
cd backend && cargo clippy -- -D warnings
```

## Roadmap

oxi is under active development. Core email functionality (read, compose, search, contacts, calendar) is implemented. Coming next:

- Sieve filter support
- Attachment previews
- Keyboard shortcuts
- Theme customization
- Multi-account support

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Commit your changes
4. Open a pull request against `main`

## License

[MIT](LICENSE)

---

<div align="center">

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=c0h1b4/oxi&type=Date)](https://star-history.com/#c0h1b4/oxi&Date)

</div>
