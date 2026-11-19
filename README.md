# OxideChat

OxideChat is a modern, performance and on customization focused AI chat application.

More tbd...

# Deployment

OxideChat ships as one Docker image, `ghcr.io/nxtcore/oxidechat`, built for `linux/amd64` and `linux/arm64`. It contains the API, the web frontend and nginx, and serves plain HTTP on port 8080. PostgreSQL runs separately.

Every release tag `vX.Y.Z` publishes `X.Y.Z` and moves `latest`; pre-releases such as `v1.0.0-rc.1` only publish their own tag. Pin a version in production.

## First start

```bash
curl -O https://raw.githubusercontent.com/NxtCore/OxideChat/master/docker-compose.yml
curl -o .env https://raw.githubusercontent.com/NxtCore/OxideChat/master/.env.example
```

Edit `.env`: set `POSTGRES_PASSWORD`, set `BASE_URL` to the public `https://` URL, and set `ENCRYPTION_KEY` (`openssl rand -hex 32`). Then:

```bash
docker compose up -d
```

Open `BASE_URL`; the setup screen creates the first administrator account. See `.env.example` for every setting.

What must survive an update:

- **The encryption key**: `ENCRYPTION_KEY`, or `/app/data/master.key` when it is unset. Without it, stored provider API keys and OAuth secrets cannot be decrypted. Back it up.
- **The database**.
- **`/app/uploads`** when `IMAGE_STORAGE_TYPE=file`.

## Updating

```bash
docker compose pull
docker compose up -d
```

Back up the database first. Images older than 0.4.2 kept the generated key inside the container at `/app/.data/master.key`: before updating from one, copy it out (`docker compose cp oxidechat:/app/.data/master.key .`) and set its content as `ENCRYPTION_KEY`. Database migrations run automatically on start; if one fails, or takes longer than 30 seconds, the container exits and the logs (`docker compose logs oxidechat`) say why. Downgrading after a migration is not supported: restore the backup instead.

## Reverse proxy and security

- Put a TLS-terminating proxy (Caddy, Traefik, nginx, ...) in front and set HSTS there; the container itself speaks plain HTTP.
- Session cookies are `Secure` unless `BASE_URL` starts with `http://`, so only use `http://` for local setups.
- `docker-compose.yml` publishes the container on `127.0.0.1:8080` only, so it is reachable through your proxy on the same host and not directly from the network. Change the port mapping only if the proxy runs on another machine.
- Set `TRUSTED_PROXIES` to the address or range your proxy connects from (comma-separated, for example `172.18.0.1` for a proxy on the Docker host, or the proxy container's network such as `172.18.0.0/16`). Only requests from these addresses may name the client IP in `X-Forwarded-For`. It is empty by default: every request then counts as coming from the proxy's address, so all users share the per-IP rate limits until you set it. `docker compose logs oxidechat` shows the address requests arrive from.
- The bundled `nginx.conf` adds baseline security headers (nosniff, frame-deny, referrer policy, CSP), a 25 MB request-body cap, and per-IP rate limits on `/api/v1/auth/` and the gateway. The OpenAI-compatible gateway is served under `/openai/`, the Anthropic Messages gateway (for Claude Code) under `/anthropic/`.
- Base images in the `Dockerfile` are pinned by digest, and CI signs pushed images keylessly with cosign (GitHub OIDC). Verify with `cosign verify <image>@<digest>`.

# Inspiration
This is an example coming from the following projects:


- [T3 Chat](https://t3.chat/)
    - And resulting from a coding competion: [intern3 chat](https://github.com/intern3-chat/intern3-chat)
- [OpenWeb UI](https://github.com/open-webui/open-webui)

# License

OxideChat is licensed under the [Apache License 2.0](LICENSE), except for the `ee/` directory, which has its own license (see [ee/README.md](ee/README.md)).
