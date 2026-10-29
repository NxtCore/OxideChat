# OxideChat end-to-end tests

Playwright tests that drive the real UI against the real backend, an isolated Postgres database and a
fake OpenAI-compatible provider.

## What runs

| Port | Process | Source |
| ---- | ------- | ------ |
| 3100 | Proxy that routes `/api`, `/openai` and `/health` like nginx does in production | `scripts/proxy.mjs` |
| 3101 | Backend binary (`target/debug/OxideChat`) | `scripts/start-backend.mjs` |
| 3102 | Nuxt production build | `scripts/start-frontend.mjs` |
| 3103 | Fake LLM provider | `scripts/mock-llm.mjs` |

Every run drops and recreates the `oxidechat_e2e` database, so the first test walks through the real
first-run setup page. The backend never touches the database from `.env`: the start script overrides all
`POSTGRES_*` variables, and it refuses to reset a database whose name does not contain `e2e`.

## Deterministic costs

The fake provider always reports **1000 input and 2000 output tokens**. The seed step prices every
mock model at **$10 / $20 per million tokens**, so each successful request costs exactly
**$0.05** (`MOCK_COST_PER_REQUEST` in `tests/support/constants.ts`). That makes usage, request logs and
budgets checkable to the cent.

Mock models:

- `mock-chat` echoes the last user message as `Mock reply: <message>`
- `mock-error` returns an upstream 500
- `mock-slow` streams slowly, for stop/interrupt tests

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

## Built-in checks

Every test fails when the page throws, logs a `console.error`, or receives an HTTP 5xx response.
A test that expects one of these calls `allowErrors(/pattern/)`. `pages.spec.ts` visits every
page as admin and as a member, and fails when an untranslated i18n key such as `settings.foo.bar`
is visible.

Locators use translations loaded from `/api/v1/base` through the `t()` fixture, so changing English
copy does not break tests.
