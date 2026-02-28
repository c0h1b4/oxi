# PROJECT KNOWLEDGE BASE

**Generated:** 2026-02-26
**Commit:** 13fcf57
**Branch:** main

## OVERVIEW

Modern webmail client — React/Next.js frontend + Rust/Axum backend. Connects to any IMAP/SMTP server (not a mail platform). SQLite per-user cache + Tantivy full-text search. IMAP credential auth.

## STRUCTURE

```
oxi-email/
├── backend/           # Rust Axum API server
│   ├── src/           # Main crate (routes, db, imap, smtp, auth, search)
│   └── migrations/    # Refinery SQL migrations
├── frontend/          # Next.js 16 App Router
│   └── src/
│       ├── app/       # Routes (auth)/mail, login
│       ├── components/# mail/, contacts/, settings/, shared/, ui/
│       ├── hooks/     # Data fetching (TanStack Query)
│       ├── stores/    # Zustand stores (auth, UI, compose)
│       └── lib/       # API client, utilities
├── run.sh             # Build + run script (./run.sh reset-db to clear cache)
└── docker-compose.yml # Container deployment
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| API endpoints | `backend/src/routes/` | mod.rs has router; auth.rs, messages.rs, folders.rs |
| IMAP operations | `backend/src/imap/client.rs` | RealImapClient + MockImapClient |
| SMTP sending | `backend/src/smtp/client.rs` | lettre-based |
| Auth/session | `backend/src/auth/` | IMAP credential validation, session store |
| Database layer | `backend/src/db/` | SQLite via rusqlite + refinery migrations |
| Search indexing | `backend/src/search/engine.rs` | Tantivy full-text |
| Real-time | `backend/src/realtime/` | WebSocket + IMAP IDLE |
| Mail UI | `frontend/src/components/mail/` | ComposeDialog, ReadingPane, MessageList |
| State management | `frontend/src/stores/` | Zustand: useAuthStore, useUiStore, useComposeStore |
| Data fetching | `frontend/src/hooks/` | TanStack Query hooks |
| API client | `frontend/src/lib/api.ts` | apiGet, apiPost, apiPatch, apiDelete |

## CONVENTIONS

**Backend (Rust)**
- Layered module structure: routes → db/imap/smtp → domain
- Error handling via `AppError` enum → JSON envelope `{error: {code, message, status}}`
- Config via figment: env vars map to `AppConfig` fields (e.g., `IMAP_HOST` → `imap_host`)
- All protected routes require `auth_guard` middleware + CSRF

**Frontend (TypeScript)**
- App Router with route groups: `(auth)/mail`, `login/`
- Path alias: `@/*` → `./src/*`
- Components use shadcn/ui (Radix + Tailwind)
- Stores: Zustand for client state; TanStack Query for server state
- API calls throw on error: `throw new Error(data.error?.message)`

## COMMANDS

```bash
# Development
./run.sh              # Build frontend + backend, start server
./run.sh reset-db     # Clear SQLite cache + search indexes

# Frontend only
cd frontend && bun dev

# Backend only
cd backend && cargo run

# Tests
cd backend && cargo test
cd frontend && bun test  # Vitest

# Lint
cd backend && cargo clippy -- -D warnings
cd frontend && bun run lint
```

## ENVIRONMENT

```bash
# Required
IMAP_HOST=mail.example.com
SMTP_HOST=smtp.example.com

# Optional (defaults shown)
PORT=3001
IMAP_PORT=993
SMTP_PORT=587
TLS_ENABLED=true
DATA_DIR=/data
SESSION_TIMEOUT_HOURS=24
RUST_LOG=info
```

## NOTES

- Backend edition = 2024 (Rust 2024)
- Frontend uses bun, Node 22
- Each user gets their own SQLite DB under `DATA_DIR/{email_hash}/`
- Real-time updates via WebSocket + IMAP IDLE manager
- CI: `.github/workflows/ci.yml` runs clippy, tests, lint, build in parallel
