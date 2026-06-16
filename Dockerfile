FROM oven/bun:1-debian AS frontend-builder

WORKDIR /frontend

COPY frontend/package.json frontend/bun.lock ./
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile

COPY frontend/ ./
RUN bun run build

# -----------------------------------------------------------------------------

FROM rust:1.96-slim AS chef

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

WORKDIR /app

# -----------------------------------------------------------------------------

FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN cargo chef prepare --recipe-path recipe.json

# -----------------------------------------------------------------------------

FROM chef AS deps-builder

COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

# -----------------------------------------------------------------------------

FROM deps-builder AS rust-builder

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY migrations/ ./migrations/

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp /app/target/release/OxideChat /app/oxidechat

RUN apt-get update && apt-get install -y --no-install-recommends binutils \
    && strip /app/oxidechat \
    && rm -rf /var/lib/apt/lists/*

# -----------------------------------------------------------------------------

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    nginx \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 1000 app \
    && useradd -u 1000 -g app -s /bin/bash -m app \
    && mkdir -p /var/cache/nginx /var/log/nginx /var/lib/nginx /run /tmp/nginx \
    && mkdir -p /app/uploads/images \
    && chown -R app:app /var/cache/nginx /var/log/nginx /var/lib/nginx /run /tmp/nginx /app/uploads

COPY --from=frontend-builder /usr/local/bin/bun /usr/local/bin/bun

WORKDIR /app

COPY --chown=app:app nginx.conf /etc/nginx/nginx.conf
COPY --from=rust-builder --chown=app:app /app/oxidechat ./oxidechat
COPY --from=frontend-builder --chown=app:app /frontend/.output ./.output
COPY --chown=app:app migrations/ ./migrations/

RUN printf '%s\n' \
    '#!/bin/bash' \
    'set -e' \
    '' \
    '# Start Rust API in background (port 3001 to avoid conflict with nginx)' \
    'PORT=3001 ./oxidechat &' \
    'API_PID=$!' \
    '' \
    '# Start Nuxt SSR server in background' \
    'cd /app/.output && bun server/index.mjs &' \
    'NUXT_PID=$!' \
    '' \
    '# Start nginx in foreground' \
    'nginx -g "daemon off;" &' \
    'NGINX_PID=$!' \
    '' \
    '# Wait for any process to exit' \
    'wait -n' \
    '' \
    '# If any process exits, kill all and exit' \
    'kill $API_PID $NUXT_PID $NGINX_PID 2>/dev/null' \
    'exit 1' \
    > /app/start.sh && chmod +x /app/start.sh

USER app

# Image storage configuration:
# IMAGE_STORAGE_TYPE=database (default) | file
# IMAGE_STORAGE_PATH=/app/uploads/images (default for file storage)
VOLUME ["/app/uploads"]

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/start.sh"]
