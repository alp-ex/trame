# =============================================================================
# Trame Dockerfile - Multi-stage build for dev and prod
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Base image with build dependencies
# -----------------------------------------------------------------------------
FROM rust:1.83-slim AS base

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# -----------------------------------------------------------------------------
# Stage 2: Development image (with cargo-watch for hot reload)
# -----------------------------------------------------------------------------
FROM base AS dev

# Install watchexec for file watching (more stable than cargo-watch)
# Detect architecture and download appropriate binary
RUN apt-get update && apt-get install -y curl xz-utils \
    && ARCH=$(dpkg --print-architecture) \
    && if [ "$ARCH" = "arm64" ]; then \
         WATCHEXEC_ARCH="aarch64-unknown-linux-gnu"; \
       else \
         WATCHEXEC_ARCH="x86_64-unknown-linux-musl"; \
       fi \
    && curl -fsSL "https://github.com/watchexec/watchexec/releases/download/v1.25.1/watchexec-1.25.1-${WATCHEXEC_ARCH}.tar.xz" \
       -o /tmp/watchexec.tar.xz \
    && tar -xJf /tmp/watchexec.tar.xz -C /tmp \
    && mv /tmp/watchexec-1.25.1-${WATCHEXEC_ARCH}/watchexec /usr/local/bin/ \
    && rm -rf /tmp/watchexec* /var/lib/apt/lists/*

COPY . .

ENV HOST=0.0.0.0
ENV PORT=3000
ENV DATABASE_URL=/app/data/trame.db
ENV RUST_LOG=debug

EXPOSE 3000

CMD ["watchexec", "-r", "-e", "rs,toml,html,css,js", "--", "cargo", "run", "--manifest-path", "server/Cargo.toml"]

# -----------------------------------------------------------------------------
# Stage 3: Build dependencies (cached layer)
# -----------------------------------------------------------------------------
FROM base AS deps

# Copy only manifests first for dependency caching
COPY Cargo.toml ./
COPY server/Cargo.toml server/

# Create dummy source to build dependencies
RUN mkdir -p server/src && echo "fn main() {}" > server/src/main.rs
RUN cargo build --release --manifest-path server/Cargo.toml 2>/dev/null || true
RUN rm -rf server/src

# -----------------------------------------------------------------------------
# Stage 4: Build the application
# -----------------------------------------------------------------------------
FROM deps AS builder

# Copy actual source code
COPY server/src server/src
COPY web web

# Build the release binary
RUN cargo build --release --manifest-path server/Cargo.toml

# -----------------------------------------------------------------------------
# Stage 5: Production image (minimal runtime)
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS prod

WORKDIR /app

# Install only runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false trame \
    && mkdir -p /app/data \
    && chown -R trame:trame /app

# Copy binary from builder
COPY --from=builder /app/target/release/trame-server /app/trame-server

# Use non-root user
USER trame

ENV HOST=0.0.0.0
ENV PORT=10000
ENV DATABASE_URL=/app/data/trame.db

EXPOSE 10000

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/app/trame-server", "--health-check"] || exit 1

CMD ["/app/trame-server"]
