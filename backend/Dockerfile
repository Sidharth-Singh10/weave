# ---- Builder stage ----
FROM rust:1-slim-bookworm AS builder

WORKDIR /usr/src/weave-api

# Cache dependency compilation (only the manifest + lockfile first).
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Build the real binary.
COPY src ./src
RUN touch src/main.rs \
    && cargo build --release

# ---- Runtime stage ----
FROM debian:bookworm-slim AS runtime

# CA certificates so reqwest (rustls) can reach the LLM provider over HTTPS.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 weave

WORKDIR /app
COPY --from=builder /usr/src/weave-api/target/release/weave-api /usr/local/bin/weave-api

USER weave
EXPOSE 3001

# LLM provider configuration is injected at runtime:
#   OPENCODE_BASE_URL, OPENCODE_MODEL, OPENCODE_API_KEY (unset => mock mode)
CMD ["weave-api"]
