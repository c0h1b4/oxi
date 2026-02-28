# BACKEND KNOWLEDGE BASE

## OVERVIEW
Rust Axum backend for oxi-email. It manages IMAP/SMTP connections, SQLite caching, and Tantivy search.

## STRUCTURE
```
backend/src/
├── auth/          # Session store, IMAP auth, CSRF, middleware
├── db/            # SQLite pool and entity-specific queries
├── imap/          # IMAP protocol client (Real + Mock)
├── realtime/      # WebSocket, EventBus, and IMAP IDLE
├── routes/        # API endpoints and router assembly
├── search/        # Tantivy full-text indexing
├── smtp/          # Lettre-based mail submission
├── config.rs      # Figment env configuration
├── error.rs       # AppError enum to JSON envelope
└── main.rs        # Entry point
```

## WHERE TO LOOK
| Task | Location | Notes |
| :--- | :--- | :--- |
| Router Setup | `routes/mod.rs` | 1600+ lines. Assembles all sub-routers. |
| IMAP Protocol | `imap/client.rs` | 1900+ lines. Core protocol implementation. |
| Message API | `routes/messages.rs` | Handles list, get, and move operations. |
| DB Persistence | `db/messages.rs` | SQL queries for message metadata. |
| Search Logic | `search/engine.rs` | Indexing and querying Tantivy. |
| Mail Sending | `smtp/client.rs` | SMTP submission via lettre. |

## CONVENTIONS
- Return `Result<T, AppError>` for all fallible logic.
- Use `auth_guard` middleware for any route requiring a session.
- Keep database logic in `db/`. Routes shouldn't run raw SQL.
- Access IMAP through the `ImapClient` trait. This enables testing with mocks.
- Config values map from environment variables to `AppConfig` via Figment.

## COMPLEXITY HOTSPOTS
- `imap/client.rs`: Handles complex IMAP state and parsing. It's the largest file.
- `routes/mod.rs`: Massive router tree. Hard to navigate due to size.
- `realtime/idle.rs`: Manages background IMAP IDLE tasks and connection pooling.
- `db/messages.rs`: Complex SQL for threading and folder synchronization.
