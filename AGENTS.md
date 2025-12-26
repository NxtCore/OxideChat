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

## Architecture

- `src/main.rs` - Application entry point, sets up Axum server and database pool
- `src/jobs.rs` - Background job scheduler using tokio tasks
- `src/routes/` - HTTP route handlers
- `migrations/` - SQLx database migrations
- `JobState` struct contains shared application state (database pool)
- Wrap `JobState` in `Arc<JobState>` for sharing across async tasks
- Job scheduler spawns independent tokio tasks for each scheduled job
- Each job is a function returning `JobFuture` (pinned boxed future)
- Jobs share state via `Arc<JobState>` - clone the Arc when spawning, not the pool

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

## Testing

- Use `#[sqlx::test]` for database tests - provides isolated test database
- Use `#[tokio::test]` for async tests
- Organize tests in submodules by component: `mod job_scheduler { }`, `mod database { }`
- Clean up test data or use transactions that rollback
- Test error cases, not just happy paths
- Use `Result<(), Error>` return type in tests for cleaner error propagation

## Important Notes

- Always use parameterized queries (`$1`, `$2`) to prevent SQL injection
- Never use `unwrap()` or `expect()` in production code paths
- Job scheduler runs indefinitely - ensure jobs handle errors gracefully
- Database URL must be set in `.env` file as `DATABASE_URL`
- Migrations run automatically on startup
- Keep the code as simple as possible so beginners can also understand it

## Updating This Document

- Update this file when adding new modules or restructuring code
- Update when introducing new shared state patterns
- Update when adding performance-critical patterns
- Update when changing error handling strategies
- Update when modifying the job scheduler or testing approach
