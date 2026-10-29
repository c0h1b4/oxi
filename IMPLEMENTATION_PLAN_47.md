# Implementation plan: optional PostgreSQL support (#47)

Issue: https://github.com/c0h1b4/oxi/issues/47

## Implementation status

Implemented on `feat/47-postgres-support` with async SQLx for both engines.
SQLite default and legacy migration histories are preserved. PostgreSQL repository
contracts and an HTTP flow (including draft attachment upload, preview, send, and
cleanup) pass on PostgreSQL 18. Full test suites pass in both modes; Clippy passes
with warnings denied. Docker/Kubernetes/Nix execution was not available locally;
build inputs, YAML examples, CI matrix, and Nix secret-file options were updated.
Single-replica/local storage and no automatic import remain intentional limits.
The final review also extended the generation guard to periodic full/CONDSTORE
reconciliation and serialized cache flag read/modify/write after IMAP succeeds.
Regression tests pause old flag snapshots across a completed UIDVALIDITY reset.

## Goal and scope

Add PostgreSQL as an alternative to SQLite for all relational application data,
without changing the HTTP API or requiring PostgreSQL for existing installations.

Planning assumptions:
- SQLite stays the default, with existing per-user files and data preserved.
- PostgreSQL mode uses one database and shared tables, with explicit user isolation.
- The first supported Kubernetes topology is one Oxi replica with external
  PostgreSQL and persistent local storage. PostgreSQL support does not, by itself,
  make Oxi stateless or safe to scale horizontally.
- Tantivy remains the search engine. Replacing search, sharing sessions across
  replicas, distributed sync ownership, and object storage are separate work.
- No automatic SQLite-to-PostgreSQL import is included in the first release.
  Switching an existing installation requires a documented data-transfer decision;
  it must not be described as a lossless cache reset.

## Current implementation and constraints

- `backend/src/db/pool.rs` opens `{DATA_DIR}/{user_hash}/db.sqlite`. Its async
  `UserDb::run` accepts a closure over `rusqlite::Connection`, so it cannot simply
  be reused with an asynchronous PostgreSQL connection.
- The ten entity modules in `backend/src/db/` accept SQLite connections directly.
  Routes, authentication, IDLE, periodic sync, and search also open databases.
  Some route code executes SQL or accepts `rusqlite::Transaction` directly.
- There are 19 SQLite migrations. Login provisioning runs Refinery migrations in
  `auth/user_data.rs`; database opens separately track `PRAGMA user_version` and
  synchronize with legacy Refinery history in `db/pool.rs`.
- SQL uses SQLite-specific placeholders, `INSERT OR IGNORE/REPLACE`, `datetime`,
  `COLLATE NOCASE`, `instr`, scalar `MAX`, JSON functions, and `last_insert_rowid`.
- Isolation currently comes from separate files, not tenant columns. Singleton
  settings use `id = 1`; folder names, contact emails, tag/group names, and message
  keys are only unique within a user's file.
- `db/header_sync.rs` uses immediate SQLite transactions and a per-user generation
  to reject stale IMAP header batches after deletion/moves or UIDVALIDITY changes.
  This concurrency guarantee must survive the new backend.
- PostgreSQL would contain more than disposable IMAP cache: contacts, drafts,
  identities, preferences, tags, groups, and locally created calendar data are
  application-owned records. Draft attachment bytes remain on the filesystem.
- Sessions and mail credentials live in process memory; Tantivy indexes and draft
  attachment bytes live below `DATA_DIR`. Realtime events and sync task ownership
  are also process-local. IMAP connection budgets and login rate limits are per
  process, not cluster-wide. Do not persist plaintext mail passwords as part of
  PostgreSQL support; shared sessions need a separate security design.
- Missing Tantivy indexes are not automatically rebuilt from existing cached
  bodies: deep indexing selects `body_cached = 0`. PostgreSQL plus an ephemeral
  index directory can therefore lose body-search results after pod replacement.
  Keep the index on persistent storage; emptyDir support needs an explicit rebuild
  mechanism and is outside this initial scope.

## Recommended design

### Backend selection and lifecycle

- Add optional `DATABASE_URL`: unset means the existing SQLite backend;
  `postgres://` and `postgresql://` select PostgreSQL. Reject unsupported schemes
  and invalid configuration. A configured but unreachable PostgreSQL instance
  must never fall back silently to SQLite.
- Implementation decision: use SQLx Any with Tokio/TLS for both SQLite and
  PostgreSQL. This changes both drivers, but keeps one async repository API and
  explicit dialect-specific queries. Retain the SQLite files, migration SQL, and
  legacy Refinery history compatibility; remove the old driver dependencies.
- Build one database service at startup, inject it into the router and background
  workers, and shut down its pool gracefully. Do not create a pool per user or per
  request. Use a bounded connection pool; document its default and expose a
  maximum-connections setting for PostgreSQL deployments.
- Support certificate-verified PostgreSQL TLS and private-CA configuration. Never
  print connection strings or credentials in logs or client error messages.

### Domain-level persistence boundary

- Expose an asynchronous, user-scoped persistence API with methods grouped by
  existing domain modules. Use a shared SQLx Any connection context with explicit
  backend selection. Keep SQL and transaction types inside `db/`.
- Return existing domain records and preserve API field names, timestamp formats,
  missing-row behavior, validation, and ordering semantics.
- Run both backends through async SQLx (SQLite uses SQLx's connection workers).
  Keep database work off Tokio workers. Share only SQL supported by both engines;
  select explicit dialect-specific statements where behavior differs.
- Represent multi-step operations as repository methods that own transactions
  (for example: apply a header batch, remove a cached message, change a default
  identity, rename a folder). A closure taking `&Connection` is not the shared API.

### PostgreSQL tenancy and schema

- Add a `users` table keyed by the existing `user_hash`, and a non-null user key on
  every user-owned table, including metadata, settings, and join tables.
- Include the user key in primary/unique constraints and indexes: e.g.
  `(user_hash, folder, uid)` for messages, `(user_hash, email)` for contacts, and
  `(user_hash, name)` for tags/groups. Singleton settings become one row per user.
- Use composite foreign keys so group members, message tags, message folders,
  and draft attachments cannot reference another user's rows.
- Construct the user-scoped repository from authenticated session state, never
  from a request-supplied tenant key. Scope every read, mutation, join, and cleanup.
- Preserve `hash_email` behavior for compatibility in this issue. Document that
  the current identity key is email-only; separating identical email addresses on
  different IMAP hosts requires a separate account-key migration.
- Use native PostgreSQL booleans and suitable integer/date types with explicit
  adapters to existing Rust/API representations. UIDs and sizes must support the
  full `u32` range; MODSEQ storage must handle `u64` without lossy signed casts
  (for example `NUMERIC(20,0)`). Preserve existing numeric identity/template IDs.

### Migration and consistency rules

- Leave existing SQLite migration files and legacy history compatibility intact.
  Consolidate SQLite provisioning on the existing version-aware migration path,
  tested against both legacy Refinery-managed and current databases.
- Add a PostgreSQL baseline covering the final schema after SQLite V019, in a
  separate migration directory (e.g. `backend/postgres-migrations/`) so existing
  SQLite migration discovery cannot load PostgreSQL SQL.
- Run PostgreSQL migrations once per application startup, before readiness, with
  migration locking and transactional application. Concurrent startup must not
  apply the same migration twice. User provisioning inserts tenant/default rows
  idempotently; it must not run global DDL at login.
- Translate dialect-specific SQL explicitly. Use `ON CONFLICT` and `RETURNING`,
  timestamp/interval expressions, PostgreSQL equivalents for literal substring
  matching and case-insensitive comparisons, and explicit bind parameters.
- Treat SQLite `REPLACE` carefully: it deletes and reinserts rows and can cascade
  into cached bodies, tags, or members. Define intended upsert behavior and make
  both backends preserve unrelated data; do not reproduce accidental cascades.
- Preserve atomic mutations, per-user default uniqueness, cascades, folder rename
  behavior, UIDVALIDITY resets, and literal `%`/`_` search handling.
- Port the header-generation guard using a per-user row lock in a transaction.
  Both stale-batch checks and invalidations must acquire the same lock. Capture
  generation before IMAP work, validate under lock, and commit related SQL writes
  together. Do not hold database transactions across IMAP network calls.
- Preserve ordering of header/index writes and removals, including blocking Tantivy
  work, so rejected batches cannot recreate deleted entries. Add race tests; do
  not assume a SQL transaction also makes filesystem indexing transactional.

## Implementation sequence

### 1. Establish the persistence boundary (SQLite-only refactor)

Files: `backend/src/db/*`, `routes/*`, `auth/user_data.rs`, `realtime/{idle,sync}.rs`.

- Inventory production database access and move direct route SQL into `db/`.
- Introduce the injected database service and async user-scoped methods.
- Convert existing SQLite callers, including login/default identity creation,
  search enrichment, quota, send-side contact updates, and background jobs.
- Separate filesystem directory provisioning from relational data provisioning.
- Keep behavior unchanged except explicitly tested fixes needed for backend parity.

Exit: all existing tests pass; production callers no longer open SQLite directly
or depend on rusqlite connection/transaction types.

### 2. Add PostgreSQL configuration, pool, schema, and provisioning

Files: `backend/Cargo.toml`, `Cargo.lock`, `src/{config,main}.rs`, `src/db/`,
new PostgreSQL migrations, and router construction/test fixtures.

- Implement backend selection, pool lifecycle, validated config, safe errors,
  migrations, per-user initialization, and default preference/identity creation.
- Add basic connectivity and migration tests, including parallel startup and
  repeated login provisioning.

Exit: a fresh PostgreSQL database initializes correctly without creating SQLite
files; existing SQLite installations still start without configuration changes.

### 3. Implement PostgreSQL repositories and feature parity

Port in manageable domain batches:
1. Contacts, known addresses, contact groups, identities, and preferences.
2. Drafts/attachment metadata, tags, calendar events/settings/templates.
3. Folders, messages/bodies, header search, subject suggestions, sync generations,
   flag changes, moves/deletes, and background synchronization.

Keep Tantivy search and SQL metadata enrichment working together. Preserve draft
file access and send cleanup. Use the same contract tests for both backends.

Exit: every relational feature works in PostgreSQL mode; no hidden SQLite access
remains in authentication, API handlers, realtime tasks, or search paths.

### 4. Deployment, documentation, and upgrade policy

Files: `README.md`, `docker-compose.yml` (or a separate PostgreSQL example),
`Dockerfile`, `nix/backend.nix`, Nix module/docs, `.github/workflows/ci.yml`.

- Document `DATABASE_URL`, pool limits, TLS/CA setup, backups, and required roles.
  Use Kubernetes Secrets and secret-safe Nix/systemd configuration, not plaintext
  credentials embedded in manifests or the Nix store.
- Provide a PostgreSQL Compose example and a minimal single-replica Kubernetes
  example using external PostgreSQL and persistent `DATA_DIR`.
- Keep liveness independent of PostgreSQL; add a readiness check that reflects
  database availability and completed migrations. Respect `BASE_PATH` in examples.
- Include PostgreSQL migrations in packaged/Nix build inputs; verify the Docker
  image can connect using TLS without extra runtime tooling.
- Explain remaining local state, restart-triggered login loss, backups of draft
  files, and the unsupported multi-replica topology. Do not share a writable Tantivy
  directory between pods as a substitute for distributed search.
- Document that selecting PostgreSQL starts with an empty relational store. IMAP
  caches can be rebuilt, but application-owned data needs transfer. Do not delete
  SQLite files. Track a separate explicit importer if existing-user migration is
  required; it must preserve records, attachment bytes/paths, and generated IDs.

## Validation and acceptance criteria

- Run `cargo test` and `cargo clippy -- -D warnings`; add a CI PostgreSQL service
  and execute the shared contract/API suites against real PostgreSQL. Keep the
  default SQLite test workflow usable without a PostgreSQL server.
- Test fresh/upgrade/idempotent migrations, legacy SQLite migration history,
  concurrent initialization, failed migration startup, and database recovery.
- Test two users with identical folder names, UIDs, emails, group/tag names, and
  singleton IDs. Reads, joins, searches, updates, deletes, cascades, and bulk jobs
  must remain isolated; cross-user foreign-key inserts must fail.
- Test domain CRUD, defaults, timestamp round trips, numeric boundaries, pagination,
  case-insensitive/literal search, known addresses, and search result enrichment.
- Test message/header upserts preserve bodies/tags; folder changes preserve valid
  relationships; default selection and multi-step changes remain atomic.
- Run stale-header-versus-delete/move/UIDVALIDITY race tests on both backends and
  verify no resurrected rows or search entries. Verify database work leaves the
  Tokio runtime responsive.
- Exercise login, sync, search, compose with attachment, send, contacts, preferences,
  and calendar in PostgreSQL mode; verify no `db.sqlite` file is created.
- Replace the pod in a single-replica deployment while preserving its volume:
  relational records, attachment bytes, and indexed body-search results survive;
  documented re-login behavior remains expected. Test missing/unwritable local
  storage separately from database failure. Confirm readiness fails on database
  loss while liveness stays healthy.
- Existing SQLite data, API responses, and the default Docker deployment remain
  compatible. No frontend feature changes are expected.

## Delivery recommendation

Split into reviewable changes: (1) SQLite persistence refactor and contract tests;
(2) PostgreSQL schema/configuration plus repositories in domain batches;
(3) full integration, deployment examples, CI, and operator documentation.
Do not advertise PostgreSQL support as complete until every relational feature
uses the selected backend. Treat horizontal scaling and lossless database import
as explicit follow-up scopes, not implied benefits of this issue.
