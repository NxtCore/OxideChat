# Provider Billing Overview

## Status

Implementation-ready plan for displaying billing and credit information for every system provider configured by an OxideChat administrator.

## Objective

Add a billing summary to the admin Providers page that clearly distinguishes:

- Data reported by the upstream provider.
- Spend calculated from OxideChat usage events.
- OxideChat-enforced user and team budgets.

Every configured provider must have a useful status. Providers without a supported upstream billing API fall back to locally tracked OxideChat spend and must never be presented as having a verified upstream balance.

## Scope

### Included

- System providers where `providers.owner_id IS NULL`.
- Billing cards on the existing admin Providers page.
- Current-month local spend for every configured provider.
- Live upstream billing for OpenRouter.
- Live upstream billing for OpenAI when a separate Admin API key and project ID are configured.
- Explicit unsupported/setup-required states for Anthropic, Google, OpenAI-compatible, and custom providers.
- Secure storage of separate billing credentials.
- Cached last-known billing snapshots.
- Manual and scheduled refresh.
- Admin permissions, tests, migrations, and English/German translations.

### Not included

- User-owned provider accounts.
- Changing or enforcing limits on an upstream provider.
- Treating upstream soft thresholds as hard limits.
- Replacing the existing OxideChat budget system.
- Google Cloud Billing or Anthropic Admin API integrations in the first implementation.
- Discovering arbitrary custom-provider billing APIs.

## Terminology

- **Provider billing**: credits, costs, or thresholds reported by OpenAI, OpenRouter, or another upstream provider.
- **Local spend**: costs recorded in `usage_events` for requests made through OxideChat.
- **OxideChat budget**: limits assigned to OxideChat users or teams and enforced by OxideChat.
- **Threshold**: a soft upstream notification amount. It is not necessarily an enforced cap.
- **Balance**: prepaid credits that are actually depleted by the provider.

## Provider support matrix

| Provider kind | First implementation | Upstream credential | Display semantics |
| --- | --- | --- | --- |
| `OPENROUTER` | Supported | Existing inference key for key-level limits; optional management key for account totals | Key limit, remaining limit, monthly usage, or account credit balance |
| `OPENAI` | Supported with setup | Separate OpenAI Admin API key and project ID | Current-month project spend and configured spend-alert thresholds |
| `ANTHROPIC` | Setup required | Separate Anthropic Admin API key | Local spend until an Anthropic cost-report adapter is added |
| `GOOGLE` | Unsupported initially | Google Cloud/AI Studio billing authorization, not the Gemini inference key | Local spend only |
| `OPENAI_COMPAT` | Unsupported | Provider-specific | Local spend only |
| `CUSTOM` | Unsupported | Provider-specific | Local spend only |

## User experience

### Provider card

Add a compact billing section to every configured provider card:

```text
OpenAI                                      Connected

Provider reported
$27.40 spent this month

$72.60 remaining to the $100.00 threshold
[███████░░░░░░░░░░░░░] 27%

OxideChat tracked: $24.80 this month
Updated 3 minutes ago                         Refresh
```

Possible badges:

- `Provider reported`
- `OxideChat tracked`
- `Billing setup required`
- `Unsupported by provider`
- `Last update failed`
- `Stale`

### Provider-specific wording

- OpenRouter uses **credits**, **key limit**, and **credits remaining** when those values are returned.
- OpenAI uses **spend**, **spend threshold**, and **remaining to threshold**. Never label an OpenAI threshold as credits or a hard cap.
- Locally calculated values use **OxideChat tracked**.
- Unsupported integrations show the local value without an empty or broken upstream card.

### Configuration dialog

Add a `Billing` tab beside the current provider settings.

OpenAI fields:

- Enable upstream billing.
- OpenAI Admin API key.
- OpenAI project ID.
- Optional project display name.
- Test billing access.
- Remove stored billing credential.

OpenRouter fields:

- Enable upstream billing.
- Use the existing provider key for key-level data by default.
- Optional management key for account-wide purchased and used credits.
- Test billing access.
- Remove stored management key.

Other providers show a translated explanation and local tracking status. They do not display non-functional credential inputs.

### Failure behavior

- Failure to refresh one provider must not prevent other providers from rendering.
- A failed refresh retains the last successful snapshot and marks it stale.
- Authentication failures show setup-required status without exposing upstream response bodies.
- The normal provider list must not wait for upstream HTTP calls.

## Data model

Create a migration named similarly to:

```text
migrations/<timestamp>_provider_billing.sql
```

### `provider_billing_connections`

```sql
CREATE TABLE provider_billing_connections (
    provider_id UUID PRIMARY KEY REFERENCES providers(id) ON DELETE CASCADE,
    credential TEXT,
    external_scope_id TEXT,
    external_scope_name TEXT,
    is_enabled BOOLEAN NOT NULL DEFAULT false,
    last_status TEXT NOT NULL DEFAULT 'NOT_CONFIGURED',
    last_error_code TEXT,
    last_synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT provider_billing_status_check CHECK (
        last_status IN (
            'NOT_CONFIGURED',
            'AVAILABLE',
            'UNSUPPORTED',
            'UNAUTHORIZED',
            'UPSTREAM_ERROR'
        )
    )
);
```

`credential` contains only the separate upstream billing credential. It must never reuse the inference-key column implicitly.

### `provider_billing_snapshots`

```sql
CREATE TABLE provider_billing_snapshots (
    provider_id UUID PRIMARY KEY REFERENCES providers(id) ON DELETE CASCADE,
    metric_kind TEXT NOT NULL,
    currency TEXT NOT NULL,
    period_start TIMESTAMPTZ,
    period_end TIMESTAMPTZ,
    limit_amount NUMERIC,
    spent_amount NUMERIC,
    remaining_amount NUMERIC,
    is_hard_limit BOOLEAN NOT NULL DEFAULT false,
    thresholds JSONB NOT NULL DEFAULT '[]'::jsonb,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    fetched_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT provider_billing_metric_kind_check CHECK (
        metric_kind IN ('CREDIT_BALANCE', 'KEY_LIMIT', 'SPEND_THRESHOLD', 'SPEND_ONLY')
    )
);
```

Use `NUMERIC`/`Decimal` for monetary values. Do not use floating-point database or Rust values for normalized amounts.

### Translation entries

The migration must add English and German translations for:

- Billing tab and section titles.
- Provider-reported and OxideChat-tracked labels.
- Spent, remaining, threshold, credits, period, and refresh labels.
- Status badges and error states.
- Encryption-required message.
- OpenAI Admin key and project ID descriptions.
- OpenRouter management key description.

Do not add static user-facing strings to Vue or Rust handlers.

## Backend types

Extend `src/types/providers/` with a real billing boundary:

```text
src/types/providers/billing.rs
src/types/providers/billing_repository.rs
```

Keep request types in `requests.rs` and response types in `responses.rs`.

### Requests

```rust
pub struct UpdateProviderBillingRequest {
    pub is_enabled: bool,
    pub credential: Option<String>,
    pub external_scope_id: Option<String>,
    pub external_scope_name: Option<String>,
}
```

An omitted credential preserves the existing credential. A dedicated remove action deletes it; an empty string must not ambiguously overwrite or preserve it.

### Responses

```rust
pub struct ProviderBillingOverviewResponse {
    pub provider_id: Uuid,
    pub provider_kind: ProviderKind,
    pub status: ProviderBillingStatus,
    pub has_billing_credential: bool,
    pub external_scope_id: Option<String>,
    pub external_scope_name: Option<String>,
    pub upstream: Option<ProviderBillingMetricResponse>,
    pub local: ProviderLocalSpendResponse,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub is_stale: bool,
    pub error_code: Option<String>,
}

pub struct ProviderBillingMetricResponse {
    pub metric_kind: ProviderBillingMetricKind,
    pub currency: String,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
    pub limit_amount: Option<Decimal>,
    pub spent_amount: Option<Decimal>,
    pub remaining_amount: Option<Decimal>,
    pub is_hard_limit: bool,
    pub thresholds: Vec<Decimal>,
}

pub struct ProviderLocalSpendResponse {
    pub currency: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub spent_amount: Decimal,
}
```

Serialize enums as `SCREAMING_SNAKE_CASE`, matching existing provider conventions.

### Repository methods

Add provider-owned methods using `&PgPool`:

- `Provider::list_billing_overviews_for_admin`
- `Provider::find_billing_connection_for_admin`
- `Provider::upsert_billing_connection_for_admin`
- `Provider::delete_billing_connection_for_admin`
- `Provider::save_billing_snapshot`
- `Provider::mark_billing_refresh_failure`
- `Provider::list_enabled_billing_connections`

All SQL remains in `src/types/providers/`. Routes contain no SQL.

Add one batched usage query to `src/types/usage/repository.rs`:

- `UsageEvent::current_month_spend_by_provider(pool, provider_ids)`

The query must use `provider_id = ANY($1)`, group by provider, and return zero for providers with no usage. Avoid one query per provider.

## Upstream billing adapters

Create:

```text
src/utils/provider_billing/mod.rs
src/utils/provider_billing/openai.rs
src/utils/provider_billing/openrouter.rs
```

Define an internal normalized result and error:

```rust
pub struct ProviderBillingMetric { /* normalized Decimal fields */ }

pub enum ProviderBillingError {
    Unsupported,
    MissingConfiguration,
    Unauthorized,
    RateLimited,
    InvalidResponse,
    RequestFailed,
}
```

Use `thiserror`. Do not return raw upstream bodies or credentials in errors. Provider clients accept borrowed configuration wherever ownership is unnecessary.

### Shared HTTP behavior

- Reuse a `reqwest::Client` with a 10-second request timeout.
- Set provider-specific authorization headers.
- Treat `401` and `403` as `Unauthorized`.
- Treat `429` as `RateLimited` without retrying in the request handler.
- Reject non-success responses before deserialization.
- Deserialize only required fields.
- Never log request headers, API keys, or complete upstream error bodies.
- Store only normalized snapshots and non-sensitive details.

### OpenRouter adapter

Key-level mode:

- Call `GET https://openrouter.ai/api/v1/key` with the existing decrypted inference key.
- Read `limit`, `limit_remaining`, `limit_reset`, and `usage_monthly`.
- Return `KEY_LIMIT` when a key limit exists.
- Return `SPEND_ONLY` when the key is unlimited.

Account-level mode:

- Call `GET https://openrouter.ai/api/v1/credits` with the separately stored management key.
- Read `total_credits` and `total_usage`.
- Calculate `remaining_amount = max(total_credits - total_usage, 0)`.
- Return `CREDIT_BALANCE`.

When a management key is configured, account-level data takes precedence. Do not silently fall back after an authorization failure; surface the credential failure so the admin can correct it.

### OpenAI adapter

Requirements:

- Separate decrypted Admin API key.
- Non-empty OpenAI project ID.

Current-month cost:

- Use UTC calendar-month boundaries.
- Call `GET https://api.openai.com/v1/organization/costs`.
- Filter to the configured project and request appropriate time buckets.
- Follow pagination until complete.
- Sum normalized USD cost values for that project.

Thresholds:

- Call `GET https://api.openai.com/v1/organization/projects/{project_id}/spend_alerts`.
- Follow pagination until complete.
- Convert `threshold_amount` from cents to dollars using `Decimal`.
- Sort and deduplicate thresholds.
- Expose all thresholds.
- Use the greatest threshold as `limit_amount` for the compact progress display.
- Set `metric_kind = SPEND_THRESHOLD` and `is_hard_limit = false`.
- If no threshold exists, return `SPEND_ONLY` with current-month spend.
- Calculate remaining only when a threshold exists.

The UI must call the selected amount a **threshold**, not a budget or enforced cap.

## Service workflow

Add a service function in `src/utils/provider_billing/mod.rs`:

```rust
pub async fn refresh_provider_billing(
    pool: &PgPool,
    provider: &Provider,
) -> Result<ProviderBillingMetric, ProviderBillingError>
```

Workflow:

1. Load the provider billing connection.
2. Validate required credential and scope fields for the provider kind.
3. Decrypt the required key only immediately before the request.
4. Call the provider adapter.
5. Save the normalized snapshot.
6. Mark the connection available and clear the prior error.
7. Return the normalized metric.
8. On failure, update only status/error metadata and retain the last successful snapshot.

No database transaction should remain open during an upstream HTTP request.

## API routes

Add routes in `src/routes/mod.rs`:

```text
GET    /api/v1/admin/providers/billing
GET    /api/v1/admin/providers/{id}/billing
PUT    /api/v1/admin/providers/{id}/billing
DELETE /api/v1/admin/providers/{id}/billing
POST   /api/v1/admin/providers/{id}/billing/refresh
```

Handlers belong in `src/routes/admin/providers.rs` unless the file would exceed quality limits; if so, add `src/routes/admin/provider_billing.rs` and export it from `src/routes/admin/mod.rs`.

Permissions:

- Read overview/config status: `admin.providers.view`.
- Configure, remove, test, or refresh billing access: `admin.providers.edit`.

Behavior:

- `GET /providers/billing` reads cached snapshots plus batched local spend and performs no upstream calls.
- `GET /providers/{id}/billing` returns configuration metadata, cached snapshot, and local spend but never returns credentials.
- `PUT` validates the provider kind, scope, and encryption requirement before storing.
- `DELETE` removes the billing connection and snapshot but does not modify the provider inference key.
- `POST .../refresh` performs one live refresh and returns the updated overview.

## Credential security

OpenAI and Anthropic Admin keys have substantially broader access than inference keys.

Required safeguards:

- Reject storing a separate billing credential when `utils::encryption::is_enabled()` is false.
- Encrypt billing credentials with `encrypt_api_key` before repository calls.
- Never place billing credentials in `ProviderResponse`, logs, snapshots, or frontend state.
- Return only `has_billing_credential`.
- Preserve an existing credential when the update request omits it.
- Remove credentials through the dedicated `DELETE` endpoint.
- Do not place credentials in `extra_headers`.
- Document `ENCRYPTION_KEY` as mandatory for upstream billing connections.

## Scheduled refresh

Extend `src/jobs.rs` with a provider billing refresh job.

- Default interval: 15 minutes.
- Refresh only enabled billing connections.
- Limit concurrent upstream refreshes to four.
- Clone `Arc<JobState>` only for spawned tasks; pass `&state.db` otherwise.
- Log provider ID/name, status, and error category only.
- Continue after individual provider failures.
- The first run may occur after startup rather than delaying server startup.

Use cached snapshots for page rendering. Manual refresh remains available for immediate updates.

Mark a snapshot stale when:

- It is older than 30 minutes, or
- The latest refresh failed after the snapshot was captured.

## Frontend implementation

### Types and state

Add:

```text
frontend/types/providers.ts
frontend/stores/providerBillingStore.ts
```

The store must provide:

- `fetchBillingOverviews()`
- `fetchProviderBilling(providerId)`
- `updateProviderBilling(providerId, payload)`
- `removeProviderBilling(providerId)`
- `refreshProviderBilling(providerId)`

Keep loading/error state keyed by provider ID so one provider cannot block every card.

### Providers page

Update `frontend/pages/settings/providers.vue`:

- Fetch provider configurations and cached billing overviews in parallel.
- Attach billing state by provider ID.
- Render local spend for every configured provider.
- Render upstream values only when status and snapshot support them.
- Add a billing progress bar using theme-aware Tailwind classes.
- Add last-updated and stale states.
- Add a refresh action that disables only the selected provider.
- Add the Billing tab to the configuration dialog.
- Use ShadCN-prefixed components where auto-injected components are used.
- Use `store.getTranslation()` for every user-facing string.
- Use semantic Tailwind colors such as `text-muted-foreground`, `bg-primary`, and `bg-destructive`.

### Formatting rules

- Format amounts using `Intl.NumberFormat` and the response currency.
- Parse serialized Rust decimals carefully; do not assume they arrive as JavaScript numbers.
- Clamp progress visuals to 0–100%, but continue displaying actual spent values above a threshold.
- Show `Over threshold by ...` when remaining is zero and spend exceeds a soft threshold.
- Do not show a reset date unless the upstream response provides a meaningful period end.

## Testing

### Database tests

Create `src/tests/providers.rs` and register it in `src/tests/mod.rs`.

Use `#[sqlx::test]` for:

- Creating, updating, and deleting billing connections.
- Preserving a credential when omitted from an update.
- Preventing billing data access for user-owned providers.
- Cascading billing rows when a provider is deleted.
- Returning cached snapshots with no usage.
- Batching current-month local spend across multiple providers.
- Excluding usage outside the UTC month.

### Adapter tests

Use a local Axum mock server and configurable adapter base URLs. Cover:

- OpenRouter limited key.
- OpenRouter unlimited key.
- OpenRouter account credits.
- OpenAI cost pagination.
- OpenAI spend-alert pagination.
- OpenAI multiple thresholds, duplicate thresholds, and no thresholds.
- Currency and cents conversion.
- `401`, `403`, `429`, timeout, invalid JSON, and server error responses.
- Sanitized errors that contain no credential or upstream body.

### Route tests

Cover:

- Authentication and view/edit permission boundaries.
- Encryption-required rejection.
- Credentials never appearing in JSON responses.
- Cached list endpoint performing no upstream request.
- One provider refresh failure retaining the previous snapshot.
- Unsupported-provider response.

### Frontend verification

The repository does not currently include a frontend unit-test script. Verify through the production build and manual states:

- Available upstream data.
- Local-only fallback.
- Setup required.
- Unsupported.
- Loading.
- Stale snapshot.
- Refresh error with last-known data.
- Threshold exceeded.
- Mobile and desktop layouts.

## Verification commands

Use Bun for frontend commands.

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test
cargo sqlx migrate run
cargo sqlx prepare
cargo sqlx prepare --check
Set-Location frontend
bun run build
```

Run commands requiring `DATABASE_URL` from the repository environment where the test database is available. Commit updated `.sqlx/` metadata with SQLx macro query changes.

## Implementation order

### Phase 1: Schema and normalized contracts

1. Add billing connection and snapshot migration.
2. Add provider billing request/response/internal types.
3. Add repository methods and batched local-spend query.
4. Add database tests.

Exit criteria:

- Cached overview can represent every provider without upstream calls.
- Local current-month spend is returned for every configured system provider.

### Phase 2: OpenRouter adapter

1. Implement key-level `/api/v1/key` integration.
2. Implement optional management-key `/api/v1/credits` integration.
3. Normalize and persist snapshots.
4. Add adapter and failure tests.

Exit criteria:

- Existing OpenRouter connections show key-level usage without additional configuration.
- Management-key connections show account credits.

### Phase 3: OpenAI adapter

1. Add encrypted Admin key and project configuration.
2. Implement paginated project cost retrieval.
3. Implement paginated spend-alert retrieval.
4. Normalize soft-threshold semantics.
5. Add adapter and security tests.

Exit criteria:

- OpenAI shows current-month project spend.
- Configured thresholds and remaining-to-threshold values are correct.
- No ordinary inference key is used as an Admin key.

### Phase 4: Routes and scheduled refresh

1. Add cached overview, config, removal, and refresh routes.
2. Add permission and encryption guards.
3. Add scheduled refresh with bounded concurrency.
4. Add route tests.

Exit criteria:

- The list endpoint is fast and independent of upstream availability.
- Last-known data survives transient upstream failures.

### Phase 5: Frontend

1. Add provider billing types and store.
2. Add card summaries and state badges.
3. Add Billing configuration tab.
4. Add refresh and remove actions.
5. Add translations.
6. Build and manually verify responsive states.

Exit criteria:

- Every configured provider displays either provider-reported data or a clearly labeled local fallback.
- OpenAI never calls a soft threshold credits or a hard cap.
- One provider failure does not affect other cards.

## Acceptance criteria

- All configured system providers appear in the billing overview.
- Every provider shows current-month OxideChat-tracked spend.
- OpenRouter shows key usage/remaining data with its existing key when available.
- OpenRouter shows account credits when a management key is configured.
- OpenAI shows project spend and thresholds only after an Admin key and project ID are configured.
- Unsupported providers show local data and an explicit unsupported/setup-required state.
- Upstream, local, and OxideChat-budget data are visually and semantically distinct.
- No credential is returned to the frontend or written to logs.
- Billing credentials cannot be stored without encryption enabled.
- Cached values render without contacting upstream providers.
- Manual and scheduled refreshes retain the last successful snapshot after failures.
- Admin view/edit permissions are enforced consistently.
- SQLx metadata, Rust checks, database tests, and `bun run build` pass.

## Estimated effort

| Work | Estimate |
| --- | ---: |
| Schema, DTOs, repository, local-spend aggregation | 1 day |
| OpenRouter adapter and tests | 0.5–1 day |
| OpenAI adapter, pagination, thresholds, and tests | 1–1.5 days |
| Routes, security, caching, and scheduled refresh | 1 day |
| Frontend, translations, responsive states | 1–1.5 days |
| Final integration and verification | 0.5 day |

Expected total: **5–6.5 engineering days** for the complete first implementation.

## Follow-up extensions

- Anthropic Admin API cost-report adapter.
- Google Cloud/AI Studio authorized billing integration when a suitable programmatic balance API and authentication flow are selected.
- User-owned provider connections as a separate project.
- Historical upstream billing charts.
- Reconciliation alerts when upstream spend differs materially from OxideChat-tracked spend.
