-- Fase 6.2: app-wide preferences. One row per setting; values are JSON so
-- typed structs on the Rust side can evolve without further schema changes.
CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
