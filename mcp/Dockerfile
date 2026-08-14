# ---- Builder stage ----
# Build context must be the repository root (workspace): docker build -f mcp/Dockerfile .
FROM rust:1-slim-bookworm AS builder

WORKDIR /usr/src/weave

# Cache dependency compilation (manifests + lockfile first).
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml ./backend/Cargo.toml
COPY mcp/Cargo.toml ./mcp/Cargo.toml
COPY crates/weave-core/Cargo.toml ./crates/weave-core/Cargo.toml
RUN mkdir -p backend/src mcp/src crates/weave-core/src \
    && printf 'fn main() {}\n' > backend/src/main.rs \
    && printf 'fn main() {}\n' > mcp/src/main.rs \
    && printf '\n' > crates/weave-core/src/lib.rs \
    && cargo build --release -p weave-mcp --features http \
    && rm -rf backend/src mcp/src crates/weave-core/src

# Build the real binary (all workspace members are present).
# `touch` all sources: docker COPY can stamp identical mtimes within the same
# second, and cargo would otherwise treat the stub-phase crate sources as
# unchanged.
COPY . .
RUN find crates/weave-core/src mcp/src backend/src -name '*.rs' -print0 2>/dev/null | xargs -0 touch \
    && cargo build --release -p weave-mcp --features http

# ---- Runtime stage ----
FROM debian:bookworm-slim AS runtime

# CA certificates so reqwest (rustls) can reach the LLM provider over HTTPS.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 weave

WORKDIR /app
COPY --from=builder /usr/src/weave/target/release/weave-mcp /usr/local/bin/weave-mcp

USER weave
EXPOSE 8010

# Transport is selected at runtime:
#   WEAVE_MCP_TRANSPORT=stdio|http   (default: stdio)
#   WEAVE_MCP_HTTP_ADDR=0.0.0.0:8010
#   WEAVE_MCP_DATABASE_URL, WEAVE_MCP_DATA_DIR, OPENCODE_* (LLM)
CMD ["weave-mcp"]
