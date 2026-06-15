# OxideChat

OxideChat is a modern, performance and on customization focused AI chat application.

More tbd...

## SQLx query metadata

Route-facing SQL should use `sqlx::query!` or `sqlx::query_as!` with explicit columns. After changing checked SQL, run:

```sh
cargo sqlx migrate run
cargo sqlx prepare
cargo sqlx prepare --check
cargo build
```

Commit the updated `.sqlx/` metadata with the code change.

CI runs the same SQLx checks against a PostgreSQL service hosted inside the workflow, so it does not depend on a developer machine or external database.

# Inspiration
This is an example coming from the following projects:


- [T3 Chat](https://t3.chat/)
    - And resulting from a coding competion: [intern3 chat](https://github.com/intern3-chat/intern3-chat)
- [OpenWeb UI](https://github.com/open-webui/open-webui)
