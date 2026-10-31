# OxideChat

OxideChat is a modern, performance and on customization focused AI chat application.

More tbd...

# Security notes

Deployment hardening notes:

- TLS terminates at the edge proxy in front of this container; the app itself serves plain HTTP on port 8080, so HSTS belongs at the edge, not in `nginx.conf`.
- `COOKIE_SECURE` defaults to `true` and keeps session cookies `Secure`; only set it to `false` for local plain-HTTP development.
- Only trust forwarded headers (e.g. a `TRUST_PROXY` setting / `X-Forwarded-*`) when the container sits behind a proxy you control.
- Base images in the `Dockerfile` are pinned by immutable digest; CI signs pushed images keylessly with cosign (GitHub OIDC) — verify with `cosign verify <image>@<digest>`.
- `nginx.conf` adds baseline security headers (nosniff, frame-deny, referrer policy, CSP), a 10 MB request-body cap, and per-IP rate limits on `/api/v1/auth/` and `/openai/`.

# Inspiration
This is an example coming from the following projects:


- [T3 Chat](https://t3.chat/)
    - And resulting from a coding competion: [intern3 chat](https://github.com/intern3-chat/intern3-chat)
- [OpenWeb UI](https://github.com/open-webui/open-webui)
