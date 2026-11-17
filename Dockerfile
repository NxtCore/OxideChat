FROM oven/bun:1-debian@sha256:93d5032fe06d163427822dc1598f420363759bb18155c9cc90eb1be05a64a19e AS frontend-builder

WORKDIR /frontend

COPY frontend/package.json frontend/bun.lock ./
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile

COPY frontend/ ./
RUN bun run build

# -----------------------------------------------------------------------------

FROM rust:1.96-slim-bookworm@sha256:e18a79fc84dfcfc3ab5ba72290398a644c135c97eaa881447fddc354ee4701a3 AS chef

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

ENV SQLX_OFFLINE=true

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY migrations/ ./migrations/
COPY .sqlx/ ./.sqlx/

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp /app/target/release/OxideChat /app/oxidechat

RUN apt-get update && apt-get install -y --no-install-recommends binutils \
    && strip /app/oxidechat \
    && rm -rf /var/lib/apt/lists/*

# -----------------------------------------------------------------------------

FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3 AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    nginx \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 1000 app \
    && useradd -u 1000 -g app -s /bin/bash -m app \
    && mkdir -p /var/cache/nginx /var/log/nginx /var/lib/nginx /run /tmp/nginx \
    && mkdir -p /app/uploads/images /app/data \
    && chown -R app:app /var/cache/nginx /var/log/nginx /var/lib/nginx /run /tmp/nginx /app/uploads /app/data

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
    '# Trust X-Forwarded-For only from the proxies the operator lists, e.g. TRUSTED_PROXIES=172.18.0.0/16,10.0.0.5' \
    ': > /tmp/nginx/real_ip.conf' \
    'for range in $(printf "%s" "${TRUSTED_PROXIES:-}" | tr "," " "); do' \
    '  case "$range" in' \
    '    *[!0-9a-fA-F:./]*) echo "Ignoring invalid TRUSTED_PROXIES entry: $range" >&2 ;;' \
    '    *) echo "set_real_ip_from $range;" >> /tmp/nginx/real_ip.conf ;;' \
    '  esac' \
    'done' \
    '' \
    '# Only nginx listens publicly; it overwrites X-Forwarded-For with the real client IP' \
    'HOST=127.0.0.1 PORT=3001 TRUST_PROXY=true ./oxidechat &' \
    'API_PID=$!' \
    '' \
    '# Start Nuxt SSR server in background' \
    'cd /app/.output && NITRO_HOST=127.0.0.1 NITRO_PORT=3000 NUXT_BASE_URL=http://127.0.0.1:3001 bun server/index.mjs &' \
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

# /app/data holds master.key (credential encryption) unless ENCRYPTION_KEY is set; losing it makes stored provider keys unreadable.
# /app/uploads holds images when IMAGE_STORAGE_TYPE=file.
ENV OXIDECHAT_DATA_DIR=/app/data
VOLUME ["/app/data", "/app/uploads"]

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

CMD ["/app/start.sh"]
