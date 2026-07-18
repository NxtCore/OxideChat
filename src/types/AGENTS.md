# AGENTS.md

## Scope

These instructions apply to `src/types/` and everything below it. Follow the root `AGENTS.md` first, then these more specific rules for DTOs, database rows, and type-owned repository logic.

The `models/` folder is the reference pattern for new or ported DTO domains.

## Folder Structure

Use this layout:

```text
src/types/<domain>/
  mod.rs          core database row type, viewer context, module declarations, public re-exports
  requests.rs     request/query/body DTOs using Deserialize
  responses.rs    response DTOs using Serialize
  rows.rs         private SQLx row structs and From<Row> for response DTOs
  repository.rs   read/create methods and route-facing queries
  patch.rs        patch/update methods and update-specific helpers
```

Only add more files when the domain has a real boundary that this layout does not cover.

## `providers_billing`

`providers_billing` should contain the main database row struct and domain-local context structs.

Example shape:

```rust
mod patch;
mod repository;
mod requests;
mod responses;
mod rows;

pub use requests::*;
pub use responses::*;

pub struct DomainViewer<'a> {
    pub user_id: &'a Uuid,
}

impl BaseType for Domain {}
```

Keep call-site imports stable by re-exporting public request and response DTOs from `providers_billing`.

Do not re-export private SQL row structs.

## Requests And Responses

Put all route input DTOs in `requests.rs`.

Put all route output DTOs in `responses.rs`.

Routes must import these DTOs from `crate::types::<domain>` or `crate::types::*`, not from route modules. Route files should contain handlers only.

Keep generic HTTP wrapper types, such as `PaginatedResponse<T>`, in `src/types/axum.rs`.

## SQL Rows And Mapping

Put private SQL row structs in `rows.rs`. These structs should match exact `SELECT` shapes used by `sqlx::query_as!`.

Map rows into response DTOs with `From<Row> for Dto`. This is the only place where flat SQL columns should be assembled into nested response shapes.

Use `sqlx::types::Json<T>` in row structs for JSON/JSONB columns, then unwrap `.0` in the `From` impl.

Prefer explicit mapping over generic projection helpers. The goal is schema-checked SQL and obvious DTO construction, not clever dynamic field lists.

## Repository Methods

Put database methods on the domain type in `repository.rs` and `patch.rs`.

Use associated functions for queries and constructors:

```rust
Domain::list_for_user(pool, viewer, page, size).await
Domain::find_for_admin(pool, id).await
Domain::create(pool, params).await
```

Use instance methods only for operations on an already loaded record.

Pass `&PgPool` or `&mut Transaction<'_, Postgres>` explicitly. Do not clone `PgPool`; it is already internally shared.

Keep SQL out of route handlers. If a route needs data, add or reuse a method in the relevant type module.

## Viewer Context

When behavior depends on the authenticated user, add an explicit viewer context:

```rust
pub struct DomainViewer<'a> {
    pub user_id: &'a Uuid,
}
```

Only include fields that a query actually consumes. Do not add speculative `is_admin`, role lists, or permission flags.

Prefer separate methods for separate audiences:

```rust
list_for_user(pool, viewer, page, size)
list_for_admin(pool, page, size, search)
find_for_user(pool, viewer, id)
find_for_admin(pool, id)
```

Do not branch on admin/user behavior inside one generic method unless the SQL and response shape are truly identical.

## SQLx Rules

Use `sqlx::query_as!` or `sqlx::query!` for route-facing reads and simple writes.

List columns explicitly. Do not use `SELECT *` in checked queries.

Use SQLx aliases for checked types and nullability:

```sql
COALESCE(config.capabilities, base.capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>"
COALESCE(config.is_favorite, false) AS "is_favorite!"
provider.kind AS "provider_kind: ProviderKind"
```

Keep SQL static when possible:

```sql
WHERE ($1::bool = true OR item.is_enabled = true)
WHERE ($2::text IS NULL OR item.name ILIKE $2 ESCAPE '\')
```

For optional search, build the optional bind value in Rust and escape `%`, `_`, and `\`.

After changing SQLx macro queries, run:

```sh
cargo sqlx migrate run
cargo sqlx prepare
cargo sqlx prepare --check
cargo build
```

Commit the updated `.sqlx/` metadata with the query change.

The GitHub workflow should host its own PostgreSQL service, run migrations, and execute `cargo sqlx prepare --check`. Do not design DTO checks that depend on a developer's local database being available in CI.

## SQL-Side Field Resolution

When a response field can come from multiple layers, resolve it in SQL with `COALESCE` instead of doing fallback logic in Rust.

For user-facing model-like data, use this order:

```text
user override -> system/default config -> base row
```

For admin-facing data, use system/default config only unless the route explicitly asks for a user-specific view.

## Patch Methods

Prefer static patch SQL for small update surfaces:

```sql
UPDATE table
SET
  display_name = COALESCE($2, display_name),
  is_enabled = COALESCE($3, is_enabled),
  updated_at = NOW()
WHERE id = $1
RETURNING ...
```

Only keep dynamic patch builders when static SQL cannot express the required semantics cleanly. If dynamic SQL survives, isolate it in `patch.rs` and cover it with a database-backed test.

For multi-step admin changes, use a transaction and make follow-up writes use the post-patch values.

## `BaseType`

`BaseType` is pagination-only. Do not add table names, aliases, field lists, or projection validation to it.

Dynamic projection helpers are not the default pattern for DTO domains. Prefer explicit query shapes and checked row structs.

## Tests

Put tests in `src/tests/<domain>.rs` and register them in `src/tests/mod.rs`.

Use `#[sqlx::test]` for database behavior. Test user-specific overrides, disabled filtering, search escaping, pagination boundaries, and patch side effects when the domain supports them.

Avoid inline tests in route handlers.

## Porting Existing Domains

When porting another type to this pattern:

1. Move public request DTOs into `requests.rs`.
2. Move public response DTOs into `responses.rs`.
3. Keep the database row type in `providers_billing`.
4. Add private checked row structs in `rows.rs`.
5. Move SQL methods into `repository.rs` and `patch.rs`.
6. Replace dynamic projections with static `query_as!` queries.
7. Update routes to import from the domain module.
8. Add or update DB-backed tests.
9. Run SQLx prepare and build checks.

Port one domain at a time. Do not restructure unrelated domains while porting a specific one.
