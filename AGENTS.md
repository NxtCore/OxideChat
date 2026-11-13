# AGENTS.md

## Performance Guidelines

- Avoid unnecessary `clone()` calls - prefer borrowing with `&T` or `&mut T`
- Use `&str` instead of `String` in function parameters unless ownership is required
- Only clone `Arc<T>` when spawning new tasks - Arc cloning is cheap but still unnecessary if not spawning
- `PgPool` is already Arc-based internally - never clone it, just use `&pool`
- Preallocate `Vec` capacity with `Vec::with_capacity(n)` when size is known
- Use iterator adapters (`.map()`, `.filter()`, etc.) instead of manual loops where possible
- Prefer `&[T]` over `&Vec<T>` in function parameters
- Use `Cow<str>` when you might need to own or borrow strings conditionally
- Avoid `to_string()` and `to_owned()` unless actually transferring ownership
- Use `query_as` or `query_as!` for type-safe database queries
- Batch database operations - avoid N+1 queries by using `ANY($1)` or `IN` clauses
- Use database transactions for multi-step operations that must be atomic
- Return `Result<T, E>` instead of unwrapping - never panic in production code
- Use `async move` blocks only when necessary - prefer borrowing in async functions
- Do not introduce `Mutex` synchronization; use ownership, channels, atomics, or lock-free primitives instead

## Architecture

- `src/main.rs` - Application entry point, sets up Axum server and database pool
- `src/i18n.rs` - Global i18n translation service (singleton pattern); `load_current_user` runs each request in the user's language (`user_preferences.language`, else the installation default), which `translate` and `Language::current()` read, so `/base` returns that language and `default_language`
- `src/jobs.rs` - Background job scheduler using tokio tasks
- `src/types/` - Shared request/response DTOs and domain types
- `src/tests/` - Unit tests organized by module (e.g., `i18n.rs`, `base.rs`)
- `src/types/budgets/` - Budget definitions, assignments (user, team, API key) and spend evaluation
    - `ledger.rs` - `BudgetLedger`: one Tokio actor reserves estimated cost at admission, reconciles once to actual cost, and refunds on drop; web chat and the gateway both reserve through `BudgetLedger::reserve_for_model`, never through their own checks, once per upstream request (web chat reserves again before every tool step, and a refused step saves the answer so far)
    - `repository.rs` - `Budget::evaluate`/`status_for`/`checks` load effective budgets for a `BudgetSubject` (optional user, optional key)
- `src/types/gateway/` - Gateway rate limiting and credentials, re-exported through `crate::types::*`
    - `limiter.rs` - Admission owned by one Tokio actor; bounded admission and unbounded release/reconciliation channels replace shared locking
    - `policy.rs` - Team and key rate rules, validated policies (`GatewayRatePolicy::from_stored`), and `GatewayLimitError`
    - `estimate.rs` - Token estimation for inference requests
    - `body.rs` - Keeps concurrency reservations alive for the lifetime of HTTP response bodies
    - `repository.rs` - Database access for credentials, policies, and model access; a key belongs to a user and is pinned to one of their teams (no projects), and authentication requires the user to still be a member of that team
    - `keys.rs` / `keys_repository.rs` - API key management DTOs, errors, and SQL (create, update, revoke, rotate, key-scoped limit rules, per-key budget, team picker); audit events are written to `audit_logs` in the same transaction; every operation takes a `KeyActor` (a `&Uuid` converts to a plain user actor): owners manage their own keys, team admins (`team_members.role = 'admin'`) and installation admins manage service-account keys (`gateway_api_keys.user_id IS NULL`, created with `service_account: true`), and team admins may also revoke members' personal keys and list all keys of their teams (`GatewayTeamKeysResponse`)
    - `admin.rs` / `admin_keys.rs` / `admin_teams.rs` / `admin_audit.rs` - Installation-admin gateway controls under `/api/v1/admin/gateway/...` gated by `admin.developer.*` permissions: all-keys listing and inspection, revoke/disable/enable, team rate rules, concurrency cap and `teams.gateway_enabled`, audit trail view and query-able events (expiring keys, budget thresholds); audit rows are written in the same transaction through `write_admin_audit`, and metadata is run through `redact_metadata`; the limiter reads team policy on every authentication so changes apply to the next request
    - `mod.rs` - Shared gateway types and constants
- `src/types/teams/` - Teams and membership; `team_members.role` is `member` or `admin` (`TeamRole`, `Team::is_admin`, `Team::set_member_role`), and replacing members or a user's teams keeps the roles of members who stay
- `src/types/roles/` - Installation roles (`roles`, `role_permissions`) managed under `/api/v1/admin/roles` and `/api/v1/admin/permissions` (`admin.roles.view` / `.edit`, page `/settings/roles`): built-in roles (`roles.is_builtin`: `admin`, `user`) cannot be renamed or deleted but their permissions can change, no update or delete may leave every role without `admin.roles.edit` (`RoleError::LockOut`; writes take a `SHARE ROW EXCLUSIVE` lock on `role_permissions` so two of them cannot race past the check), a `RoleActor` may only grant or remove permissions it holds itself (wildcards count), and every create, update and delete is audited through `AuditLog::write`; each permission constant in `src/types/permissions/mod.rs` needs a row in the `permissions` seed (a test checks it) and translations `roles.permission.<name>`, grouped by `PermissionResponse::group_of` (`roles.group.<group>`)
- `src/types/request_logs/` - `gateway_request_logs` rows: one row per gateway inference request, keyed by the `x-request-id` the client received (`add_request_id` stores a `RequestLogContext` in request extensions); successes come from `OxideCostSink`, rejections and upstream failures from `RequestLog::record_failure`, and a repeated `request_id` merges into one row
    - `finish_reason` and `time_to_first_token_ms` come from Omniference's `CostRecord` (null on failure rows; a merged row keeps non-null values)
    - Streams dropped before their body ends are logged as `interrupted` by `ReservedBody` (via `RequestLogAbort`); `/models` is not an inference request and is deliberately not logged; `internal` covers database and limiter-unavailable failures
- Service-account keys have no user: `GatewayAuthContext.user_id` is `None`, they authenticate while the team exists and has the gateway enabled, their budget subject is `BudgetSubject::service_key` (own key budgets plus the team's pooled budgets only), and `usage_events.user_id` / `gateway_request_logs.user_id` are nullable (`ON DELETE SET NULL`) so usage is attributed through `team_id` and `api_key_id`; pooled team spend counts usage events recorded for that team (gateway requests use the key's team, web chat uses the user's primary team)
- `src/utils/gateway_limits.rs` - HTTP glue only: admission for a route and Omniference-rendered OpenAI limit headers; protocol formatting belongs to Omniference, not the limiter
- `src/routes/` - HTTP route handlers (handlers only, no type definitions)
    - Handlers take `auth: AuthUser` (`src/types/auth_user.rs`, reads the session user from `RequestContext`; missing user is 401), call `auth.require(&state.db, PERMISSION).await?` (a database failure is a 500, never a 403) and return `Result<Response, AppError>`
    - `AppError` (`src/types/error.rs`) renders the standard JSON error and logs 5xx through `tracing`; use `?` on `sqlx::Error`, `ErrorCode` (`Err(ErrorCode::NotFound.into())`, `.ok_or(ErrorCode::NotFound)?`) and the gateway error enums, `AppError::unique_or(e, ErrorCode::AlreadyExists)` for unique violations, `is_unique_violation` instead of matching error text, and `json_ok(body)` for 200 responses; never `eprintln!`
    - `src/routes/admin/mod.rs` and `src/routes/public/mod.rs` each expose a `router()` that `src/routes/mod.rs` merges; `get_current_user` lives in `src/utils/auth.rs`
- `src/routes/gateway/` - OpenAI-compatible gateway handlers (moved from `src/utils/openai_gateway.rs`)
- `src/types/chat/runner/` - Agent loop for web chat (`prepare_turn`, `stream_turn`): history, request building, tool calls
    - `title.rs`: when the admin setting `title_generation` (`{enabled, model_id}`, in `/api/v1/base` and the admin PATCH) is on, a chat without a title gets one after its first reply from one short request to the chosen model (else the chat's model); it is reserved against and billed to the requesting user's budget (`oxide_request_type = title`), stored only if the chat still has no title, and sent as a `title` SSE event after `done`
    - Sampling is merged request > chat (`chats.sampling`, `ChatSampling`, set via `PUT /api/v1/chats/{id}/sampling`) > model config > model; `ChatSampling.custom` becomes Omniference's `extra_body`, which never overrides reserved keys such as `model` or `stream`
    - The tools a turn may call are the request's (or a locked preset's) plus the admin model config's `enabled_tools` (`turn_tool_ids`); `request_settings.enabled_tools` keeps only the turn's own choice
- `src/types/tools/` - One file per entity (`tool.rs`, `tool_function.rs`, `tool_execution.rs`, `wasm_blob.rs`, `mcp_server.rs`); `McpScope` (`mcp_scope.rs`) separates admin-wide and per-user MCP servers; MCP servers connect over Streamable HTTP only, and server-side `stdio` servers are refused on create and on connect until they can run in a sandbox
- Provider-reported cost: `providers.reported_cost` (`ProviderReportedCost`: unit `usd`, `currency` or `credits`, optional `source` path or `header:<name>`) becomes Omniference's `ProviderEndpoint.reported_cost` in `ai::build_engine`, which emits the cost in USD; currencies use `exchange_rates`, refreshed from the ECB by `exchange_rate_job`, and a currency without a rate falls back to token pricing
- Model prices (`ModelPricing`, `src/types/models/repository.rs`): a manual price from `model_pricing_overrides` wins, else the catalog price Omniference knows for the model's provider (models.dev plus catalog overrides, through `ai::catalog()`); endpoint rows in `gateway_model_provider_options` are display only. Manual prices are pushed into the engine's catalog keyed by provider name and model ID (`ai::build_engine` loads them all, `ModelPricing::push_to_engine` after every change), so Omniference prices every request itself and `OxideCostSink` stores its cost as is; `ModelPricing` estimates and fallback costs also go through Omniference's `compute_cost`
- `src/types/provider_routing/` - Upstream provider routing for provider kinds that route on to upstream providers (`ProviderKind::UPSTREAM_ROUTING`, `routes_upstream()`; only `OPENROUTER` today). The installation (`provider_routing: {selector_enabled, allowed_providers}` in `/api/v1/base` and the admin PATCH) and each team (`teams.provider_selector_enabled`, `teams.allowed_upstream_providers`) set whether users get the chat provider selector and which upstream providers requests may reach; `EffectiveProviderRouting::combine` intersects the allowlists and lets a team's "off" win. `ProviderRoutingMiddleware`, registered on the engine in `ai::build_engine`, restricts every request's `ChatRequestIR::provider_routing` to that allowlist (gateway: the key's team; web chat and titles: all of the user's teams) and refuses others with a 403
    - Allowlists are kept per routing provider kind (`ProviderAllowlists`, `{OPENROUTER: [...]}`; a kind without an entry is unrestricted; installation in `app_config.allowed_upstream_providers`, teams in the JSONB `teams.allowed_upstream_providers`); an allowed slug also allows its variants (`deepinfra` allows `deepinfra/fp8`)
    - `sources/`: each kind's `sync` function (dispatched by `sources::sync`) fetches its upstream providers (OpenRouter: `/v1/providers`). `POST /api/v1/admin/upstream-providers/sync` and, in a background task, every model sync of a routing provider store the result in `upstream_gateways` / `upstream_providers`; `GET /api/v1/admin/upstream-providers` lists it, falling back to the endpoint options for kinds never synced. Restrictions the routing provider itself puts on our key (OpenRouter guardrail policies) are not read
    - To add a routing provider (e.g. Vercel, Cortecs): add its kind to `UPSTREAM_ROUTING`, map `ProviderRouting` in its Omniference adapter, and add a `sync` function in `sources/` called from `sources::sync`
- Provider kinds without their own Omniference adapter (`CORTECS`, `MISTRAL`) use Omniference's OpenAI Responses adapter (`/v1/responses`) and pass `ProviderKind::catalog_slug()` as `catalog_provider_slug`, so models.dev prices are looked up for that provider; every `ProviderConfig` OxideChat builds must set it
- `src/types/presets/` - Presets (`model_presets`): a base model plus instructions, sampling (`PresetSampling`), reasoning, tools (optionally locked) and up to four starters, addressed as `presets/<slug>`
    - The slug is unique across all scopes, and `presets/` is a reserved namespace: providers cannot be named `presets` and model sync skips IDs starting with `presets/` (both also enforced by CHECK constraints)
    - Scopes: `personal` (`settings.presets.edit`), `team` (managed by team admins and installation admins, visible to members) and `installation` (`admin.presets.edit`, visible to everyone). `PresetActor` carries what a caller may manage; the scope names the owner, is set at create and never changes. On top of it a preset is shared through `PUT /api/v1/presets/{id}/assignments` (`Preset::replace_assignments`) with any number of teams and users (`model_preset_shares`) or with everyone (`model_presets.shared_with_everyone`): each team or user share holds a role (`AssignmentRole`): a `viewer` may use the preset, an `editor` may also change it (every member of an editor team; `PresetActor::can_edit`, `PresetResponse.can_edit`), and only whoever manages the owning scope may delete or share it (`PresetActor::can_manage`, `PresetResponse.can_manage`); everyone can only be a viewer, `PresetResponse.shared` marks presets the viewer does not own, a new share needs the actor to manage that team (`PresetActor::can_share_with`; users and everyone need an installation admin), and a preset that is shared or not personal may only bundle system tools. `GET /api/v1/presets/scopes` lists where a user may create presets
    - `/api/v1/admin/presets` (`admin.presets.view` / `.edit`) lists every preset and can rename or turn off any of them (`disabled_by_admin`: only an installation admin can turn it on again); every write is audited through `AuditLog::write`; the frontend shows this list as the All presets tab of `/settings/presets` (`SettingsPresetsAdminList`), and `/settings/admin-presets` redirects there
    - Gateway (`presets/gateway.rs`, `gateway/preset.rs`): `/models` lists the presets a key can call (its owner's personal presets, its pinned team's, the installation's and those shared with its owner, its team or everyone; service-account keys never see personal ones that are not shared with their team) whose base model the key's team may use; `resolve_inference` resolves `presets/…` first, budgets and pricing use the base model, the preset's prompt goes before the client's messages (Responses: before `instructions`), sampling and reasoning only fill fields the client left empty, tools are not applied, and `oxide_preset_id` lands on `usage_events.preset_id` and `gateway_request_logs.preset_id`
    - Web chat resolves a `presets/` key in `prepare_turn` to the preset's base model; budgets, pricing, access and `messages.model_id` use the base model, `request_settings.model_key` keeps the preset key, and `oxide_preset_id` lands on `usage_events.preset_id`. Sampling is merged request > chat > preset > admin model config > model, and the preset's instructions follow the admin model prompt
    - A preset never grants model access: it is usable only while its viewer may use the base model (`PresetResponse.unavailable` says why not); delete is a soft delete, and chats of a deleted preset fall back to its base model in the composer
- `src/types/assignments.rs` - The one shape for assigning something to users, teams, roles or keys: every feature exposes `GET` and `PUT …/{id}/assignments`, both returning `AssignmentsResponse` (`kinds` the feature accepts, `single`, `roles` a target can hold (empty for features without roles), `targets: [{kind, id, label, inherited, owner, role}]`; the `owner` target owns the thing and stays); the `PUT` body is `ReplaceAssignmentsRequest` (`targets: [{kind, id, role?}]`, the full list that should remain) and is checked with `is_valid_for(&Feature::ASSIGNMENT_KINDS)`; a target that does not exist is a `ValidationFailed` (`AppError::foreign_key_or`), and every replace is one audited transaction
    - Budgets (`/api/v1/admin/budgets/{id}/assignments`, users and teams; a target that gains a budget loses its other budget of the same kind; per-key budgets stay with the key), model access (`/api/v1/admin/models/{id}/assignments`, teams only, guarded by `admin.teams.*` because it changes team access; teams with all models or a provider grant are `inherited`) and presets (`/api/v1/presets/{id}/assignments`: the owner plus the teams, users or `everyone` it is shared with) use it
    - A user's own roles and teams use it too (`/api/v1/admin/users/{id}/assignments`, `User::replace_assignments`): `kinds` is empty for a viewer who may not change the account and leaves out `role` on one's own account, the default team is `inherited`, a role change needs the actor to hold every permission of the roles gained or lost (`RoleActor::check_role_change`) and a team change needs them to outrank the user
    - New features that assign to users, teams, roles or keys add the same two routes and a `Feature::assignments` / `replace_assignments` pair instead of their own request and response types
- `src/types/page.rs` - `PageQuery` (`page`, `size`) and `Page<T>` (`has_more`, `items`) for paginated endpoints
- `src/types/usage/` - Usage events, one per upstream request: web chat and the gateway both tag requests with `oxide_*` metadata (web chat: `oxide_user_id`, `oxide_model_id`, `oxide_request_type = chat`, and the step's budget reservation) and `OxideCostSink` records them from Omniference's cost record, also when a stream is dropped; request logs are only written for API-key requests. Web chat sends the provider's bare model ID like the gateway, which Omniference's catalog prices by
- `src/types/oauth/` - Sign-in providers (`oauth_providers` table): `OAuthProvider` holds endpoints, an encrypted client secret (never returned; `OAuthProviderResponse` has none), scopes, a `ClaimMapping` and per-provider flags `trust_email`, `allow_registration`, `enabled`; `provider.rs` has the SQL and the Google and Discord presets, which only prefill the admin form; `sign_in.rs` (`OAuthProvider::sign_in`) finds the account by identity, links by email only when the provider reports it verified (or `trust_email` and it says nothing), else creates it when `allow_registration`; `claims.rs` reads a provider's profile JSON through the mapping
- `src/utils/oauth.rs` / `src/utils/outbound.rs` - OAuth2 code flow with PKCE, OIDC discovery (the discovery issuer must match) and profile fetch; every request to an admin-entered URL goes through `outbound::guarded_client`, which resolves the host once, pins the client to the checked addresses, never follows redirects, always refuses link-local and metadata addresses and refuses private ones and plain `http` unless `OIDC_ALLOW_PRIVATE_HOSTS=true`
- `src/routes/admin/oauth_providers.rs` - CRUD under `/api/v1/admin/oauth-providers` (and `/presets`) gated by `admin.config.*`, audited; the login routes are `/api/v1/auth/oauth/{slug}` and `/callback`, and `/api/v1/base` lists enabled providers as `{slug, name, icon}`
- `src/types/user/overview.rs` - `User::overview` backs the per-user admin page with one request, `GET /api/v1/admin/users/{id}/overview` (`admin.users.view`): profile, sessions, roles, the effective permissions with the roles that grant each one (`EffectivePermission::resolve`; `through` names the wildcard when a role grants it through one), teams with the team role, and `assignments`; model access (`Team::model_access_for_user`: the team grants behind it, since only teams grant models), presets, budgets with their spend and API keys belong to other admin areas and are `null` unless the viewer holds that area's view permission (`admin.teams.view`, `admin.presets.view`, `admin.budgets.view`, `admin.developer.view`); `can_edit` says whether the viewer may change the account, and `DELETE /api/v1/admin/users/{id}/sessions/{session_id}` signs the user out of one session
- `src/types/user/sessions.rs` - A user's login sessions; the session ID is the login cookie's value, so clients only ever see `User::session_public_id` (its SHA-256 hash)
- `src/config.rs` - `Config::apply_update` stores admin settings (flags, default model, language, default theme, instance name and icon) in one transaction; the instance icon is an uploaded image whose ID is stored in config and served by the image route `/api/v1/images/{id}`, which skips the session check for that one image (PNG, JPEG, GIF or WebP only, checked against the bytes), `/api/v1/base` returns it as `instance: {name, icon_id}`, and the frontend reads it through `store.appName` / `store.appIcon`
- `AuditLog::write` (`src/types/gateway/admin.rs`) writes an audit row in the caller's transaction and always redacts its metadata
- `migrations/` - SQLx database migrations
- `JobState` struct contains shared application state (database pool)
- Wrap `JobState` in `Arc<JobState>` for sharing across async tasks
- Job scheduler spawns independent tokio tasks for each scheduled job
- Each job is a function returning `JobFuture` (pinned boxed future)
- Jobs share state via `Arc<JobState>` - clone the Arc when spawning, not the pool

## Code Organization

### Types Module (`src/types/`)

- All request/response structs live in `src/types/`, not in route files
- Organized by domain: `i18n.rs`, `base.rs`, etc.
- `src/types/mod.rs` re-exports all types for easy importing: `use crate::types::*;`
- Each file uses section comments for clarity:
    - Request Types (structs with `Deserialize`)
    - Response Types (structs with `Serialize`)
    - Internal Types (DB row structs, helper functions)
- Group related fields of a request or response into a nested object instead of prefixing flat field names: `instance: {name, icon_id}`, not `instance_name` and `instance_icon_id`; requests use the same shape as the response they update

### Tests Module (`src/tests/`)

- Unit tests live in `src/tests/`, not inline in route files
- One test file per module: `i18n.rs`, `base.rs`
- Use `#[cfg(test)]` wrapper for all test code
- Group related tests in submodules: `mod types { ... }`
- Import types via `crate::types::*`

### Route Files (`src/routes/`)

- Contain only handler functions, no struct definitions
- Import types from `crate::types::{...}`
- Keep handlers thin: validate → delegate → respond
- ABSOLUTLY NO direct database access in route files, all database access is handled via the data/types model

### Data Model Pattern

* **Structure:** Locate domain models and their `impl` blocks in `src/types/` (e.g., `user.rs`).
* **Logic:** Use associated functions (`::`) for queries/constructors and instance methods (`.`) for operations on specific records.
* **State:** Pass `&PgPool` explicitly as a parameter; never rely on global database state.
* **Mapping:** Implement `to_response` methods within the model to handle DTO transformations.
* **Encapsulation:** Keep all SQL logic and business rules for a type within its dedicated file.

## Code Quality

- Clippy lints are enforced: `correctness` and `suspicious` are denied, `perf` and `style` are warnings
- Cognitive complexity threshold: 15
- Max function arguments: 5 (use structs for more)
- Max function lines: 100
- Document public APIs with `///` doc comments
- Include `# Errors` and `# Panics` sections in documentation where applicable
- Use `#[must_use]` for functions whose return value should not be ignored
- Prefer explicit types over `_` in public APIs
- Use `thiserror` or similar for custom error types
- Do not use static strings, use the i18n system instead

## Testing

- Use `#[sqlx::test]` for database tests - provides isolated test database
- Use `#[tokio::test]` for async tests
- Tests live in `src/tests/` with one file per module
- Group related tests in submodules: `mod types { }`, `mod handlers { }`
- Clean up test data or use transactions that rollback
- Test error cases, not just happy paths
- Use `Result<(), Error>` return type in tests for cleaner error propagation
- Frontend DTO types and permission names are generated into `frontend/types/generated/` (committed, CI-checked): derive `#[cfg_attr(test, derive(ts_rs::TS))]` on the Rust type, add it to `src/tests/bindings.rs`, and regenerate with `cargo test generate_frontend_types`
- UI end-to-end tests live in `e2e/` (Playwright, run with Bun) against the real backend, an isolated `oxidechat_e2e` database and a real LLM provider with a cheap model (configured in `e2e/.env`; without a key it falls back to a local stand-in); see `e2e/README.md`
- New UI features get an e2e spec; locate elements through the `t()` fixture (translations) or `data-testid`, never hardcoded English text

## Frontend
- Use Tailwind classes for styling
- Use `useMainStore()` for global state management
- Shared data (chat models, tools and MCP servers, the user's budget, developer teams and keys) is loaded once and served from its store; remounts and navigation must not refetch it. A successful write through `$customFetch` marks the affected cache stale (`STALE_AFTER_WRITE` in `frontend/plugins/fetch.ts`), and the next consumer reloads it; `ensure*` loaders go through `inflight` (`frontend/utils/inflight.ts`) so callers in the same tick share one request; `/users/@me` already carries preferences including `effective_default_model_key`. `e2e/tests/request-audit.spec.ts` fails on duplicate or needless refetches
- Use Shadcn UI for components (shad-vue)
- All ShadCN components are prefixed with `Shad`, e.g., `ShadButton` or `ShadSelect`, they are auto injected
- Use the store.getTranslation() function for translations, always use them, no static strings
- Use customizable Tailwind classes like bg-primary, text-primary, etc. and not use fixed colors
- Themes live in `frontend/stores/theme.ts`: `modePreference` is what the user picked (`light`, `dark` or `system`) and `currentMode` what it resolves to; a user's own `cssVars` win over the installation's `defaultTheme`, which comes from `/api/v1/base`. Mode and selected theme URL stay in localStorage, the CSS variables and custom theme URLs are saved to `user_preferences` and the server's copy wins on load
- Settings pages render inside `SettingsPage` (`frontend/components/settings/SettingsPage.vue`: title, description, `#leading` and `#actions` slots); `frontend/pages/settings.vue` owns the scrolling and the grouped navigation, so pages must not set their own height or overflow; section switches inside a page use `ShadTabs`
- The Developer area (`frontend/pages/developer.vue`) lands on API keys and owns its scrolling like settings; gateway base URLs are listed once in `GATEWAY_ENDPOINTS` (`frontend/utils/developer.ts`), the playground's request logic lives in `usePlaygroundChat`, the Models page is a flat table filtered by provider, team and capabilities from `/api/v1/developer/models` (never grouped by parsing model IDs, whose naming has no standard), and product documentation is hosted externally rather than in the app
- Presets load once into `usePresetStore` (`frontend/stores/presetStore.ts`); `presetAsModel` (`frontend/utils/presets.ts`) turns one into a composer model whose `model_id` is the preset key and whose `preset` field carries the preset, so code that needs the real model (provider options) uses `baseModelId`
- Anything assigned to users, teams, roles or keys is picked with `SettingsAssignmentPicker` (`frontend/components/settings/AssignmentPicker.vue`: a list of rows that each say what the assignment does through the page's `describe`, an Assign popover with search and a `note` on consequences, `inherited` and `owner` rows that cannot be removed, a role dropdown per row when the feature has `roles`, and with `single` one choice per kind that `confirm` can stage before it is saved) and loaded and saved through `useAssignments` (`frontend/composables/useAssignments.ts`), which replaces the whole list with one `PUT …/assignments` and shows the toast; pages pass the candidates as `options` and the explaining copy (`description`, `describe`, `emptyHint`, `note`) and never build their own team or user pickers; a preset's sharing lives on its Sharing tab (a new preset picks its owner there, a saved one lists who it is shared with), with a badge in the page header, and presets shared with the viewer sit under the list's Shared with me tab
- The per-user admin page (`frontend/pages/settings/users/[id].vue`, opened from a row of the users list) loads `…/overview` once and shows one tab per section (`frontend/components/settings/users/*Tab.vue`, each built from `SettingsUsersSection`: heading, a link to the page where the section is changed, and the empty state); tabs of sections the response leaves `null` are hidden. Email and username save through the save bar, roles and teams through `SettingsAssignmentPicker` and `useAssignments`, after which the overview is loaded again because permissions, model access and budgets follow from them; the other sections link out (`/settings/teams?team=<id>` and `/settings/budgets?budget=<id>` open that entry)
- Fields edited directly on a settings page use `useSettingsForm` (`frontend/composables/useSettingsForm.ts`) and pass it to `SettingsPage` as `:form`, which shows the floating save bar and asks before unsaved changes are lost; a save must be atomic on the backend. Dialogs keep their own Cancel/Save footer, and quick actions on list rows apply instantly with a toast

## Important Notes

- Always use parameterized queries (`$1`, `$2`) to prevent SQL injection
- Never use `unwrap()` or `expect()` in production code paths
- Job scheduler runs indefinitely - ensure jobs handle errors gracefully
- Database URL must be set in `.env` file as `DATABASE_URL`
- Migrations run automatically on startup
- Keep the code as simple as possible so beginners can also understand it
- Do not add comments to the code itself
- When writing axum routes, please use the response "module" to create responses
- NO SQL statements in route handlers, please add a function to the specific type's module for database operations (check if a smilar function already exists)

## Local Development

- Omniference is a major part of the application; developers may point Cargo at a local checkout with a patch override in `.cargo/config.toml`
- A local reverse proxy must forward both `/api` and `/openai` to the backend, as `nginx.conf` does; otherwise the Playground gets a 404 from Nuxt

## Branches and Pull Requests

- Name branches, PR titles and PR descriptions as described in `CONTRIBUTING.md`; PR titles look like `[FEAT] - Add key expiry reminders`, and PRs are squash-merged, so the PR title becomes the commit subject on `master`

## Updating This Document

- Update this file when adding new modules or restructuring code
- Update when introducing new shared state patterns
- Update when adding performance-critical patterns
- Update when changing error handling strategies
- Update when modifying the job scheduler or testing approach