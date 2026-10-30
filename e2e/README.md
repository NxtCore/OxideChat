# OxideChat end-to-end tests

Playwright tests that drive the real UI against the real backend, an isolated Postgres database and a
real LLM provider with a cheap model.

## What runs

| Port | Process | Source |
| ---- | ------- | ------ |
| 3100 | Proxy that routes `/api`, `/openai` and `/health` like nginx does in production | `scripts/proxy.mjs` |
| 3101 | Backend binary (`target/debug/OxideChat`) | `scripts/start-backend.mjs` |
| 3102 | Nuxt production build | `scripts/start-frontend.mjs` |
| 3103 | Local stand-in provider, used only when no API key is configured | `scripts/mock-llm.mjs` |

Every run drops and recreates the `oxidechat_e2e` database, so the first test walks through the real
first-run setup page. The backend never touches the database from `.env`: the start script overrides all
`POSTGRES_*` variables, and it refuses to reset a database whose name does not contain `e2e`.

## Provider

Copy `.env.example` to `e2e/.env` and set `E2E_LLM_API_KEY`. Bun loads the file automatically. The
defaults use OpenRouter with `openai/gpt-4o-mini`; a full run sends about 20 short requests and costs
well under one cent.

Pick a model that:

- has catalog pricing, because the seed step fails otherwise and costs cannot be checked
- is not a reasoning model, because the tests cap replies at 20 tokens

Without a key, the suite falls back to `scripts/mock-llm.mjs`, an OpenAI-compatible stand-in that
rejects wrong API keys like a real provider. It runs the same specs and is useful for forks or offline
work, but it cannot catch provider-specific problems.

## How dynamic costs are checked

Token counts and prices are not hardcoded. Each test reads the usage the provider returned, then checks:

- the request log has exactly the same input and output tokens
- `cost_total` equals tokens × the model's effective pricing from `/api/v1/admin/models/{id}/pricing`
  (`tests/support/cost.ts` mirrors Omniference's formula)
- key budget `spent` equals the sum of the logged costs, and a budget set to exactly the spent amount
  blocks the next request with 429
- the Logs page and budget dialog show those same numbers

Upstream failures are real: a request with `temperature: 99` is rejected by the provider with a 400.
A separate test creates its own provider and checks that disabling, re-enabling, changing the API key
and deleting it in admin settings all apply to the very next request.

## Running locally

Requirements: Docker, Bun, Rust.

```bash
cd e2e
bun install
bunx playwright install chromium
bun run db:start
cargo build
bunx playwright test
```

Use PowerShell for `bun run db:start` on Windows, because Git Bash rewrites the `--tmpfs` path.

- `E2E_SKIP_BUILD=1` reuses the existing `frontend/.output` build, which saves about 3 minutes per run.
- `bunx playwright test --ui` opens the interactive runner.
- `bunx playwright show-report` opens the HTML report with traces, screenshots and videos of failures.

## CI

`.github/workflows/e2e.yml` reads the `E2E_LLM_API_KEY` repository secret, plus the optional
`E2E_LLM_KIND`, `E2E_LLM_BASE_URL` and `E2E_LLM_MODEL` repository variables. Without the secret it uses
the stand-in provider.

## Built-in checks

Every test fails when the page throws, logs a `console.error`, or receives an HTTP 5xx response.
A test that expects one of these calls `allowErrors(/pattern/)`. `pages.spec.ts` visits every
page as admin and as a member, and fails when an untranslated i18n key such as `settings.foo.bar`
is visible.

Locators use translations loaded from `/api/v1/base` through the `t()` fixture, or `data-testid` and
`data-role` attributes, so changing English copy does not break tests.
