# Migrations

SQL migrations applied to the local SQLite database on app startup.

## End users

End users never need `sqlx-cli`. Migrations are embedded into the binary at compile time via `sqlx::migrate!("./migrations")` (see [`src-tauri/src/db/mod.rs`](../src/db/mod.rs)) and applied automatically the first time the app runs.

## Contributors — adding a new migration

1. Install `sqlx-cli` once:

   ```
   cargo install sqlx-cli --no-default-features --features rustls,sqlite
   ```

2. From `src-tauri/`, create a new migration file:

   ```
   DATABASE_URL="sqlite://abraxas.sqlite?mode=rwc" sqlx migrate add <name>
   ```

   This creates `src-tauri/migrations/<timestamp>_<name>.sql`. Fill in the SQL.

3. Commit the new `.sql` file. `cargo build` picks it up automatically.

## Conventions

- Non-reversible migrations only (no `.up.sql`/`.down.sql` split). Rolling back a shipped desktop DB is a footgun — forward-only keeps the upgrade path unambiguous.
- Filename format: `<4-digit-sequence>_<snake_case_name>.sql` (e.g. `0002_add_conversations.sql`). Matches sqlx's lexicographic ordering.
- Keep each migration focused on one logical schema change.
