# =============================================================================
# oxi.email — Multi-stage Docker build
# =============================================================================
# Stage 1: Frontend static export (Next.js via bun)
# Stage 2: Backend release build (Rust/Axum)
# Stage 3: Minimal runtime image (Debian bookworm-slim)
# =============================================================================

# ---------------------------------------------------------------------------
# Stage 1 — Frontend build
# ---------------------------------------------------------------------------
FROM oven/bun:1 AS frontend-build

WORKDIR /app/frontend

# Copy dependency manifests first for layer caching
COPY frontend/package.json frontend/bun.lock ./
RUN bun install --frozen-lockfile

# Copy remaining frontend source and build static export
COPY frontend/ .
RUN bun run build

# ---------------------------------------------------------------------------
# Stage 2 — Backend build
# ---------------------------------------------------------------------------
FROM rust:1-bookworm AS backend-build

WORKDIR /app/backend

# Copy manifests first for dependency caching
COPY backend/Cargo.toml backend/Cargo.lock ./

# Copy migrations (needed at compile time by refinery embed_migrations! macro)
COPY backend/migrations/ migrations/

# Create a dummy main.rs so cargo can resolve and compile all dependencies.
# This layer is cached as long as Cargo.toml / Cargo.lock don't change.
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Copy real source and migrations, then rebuild. Cargo detects the source
# change and recompiles only the project crate, reusing cached dependencies.
COPY backend/src/ src/
COPY backend/migrations/ migrations/
RUN touch src/main.rs && cargo build --release

# ---------------------------------------------------------------------------
# Stage 3 — Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the release binary and frontend static files
COPY --from=backend-build /app/backend/target/release/oxi-email-server .
COPY --from=frontend-build /app/frontend/out ./static

# Configure defaults (overridable at runtime)
ENV STATIC_DIR=/app/static
ENV DATA_DIR=/data
ENV PORT=3001
ENV HOST=0.0.0.0

EXPOSE 3001

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD curl -f http://localhost:3001/api/health || exit 1

CMD ["./oxi-email-server"]
