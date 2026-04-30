CREATE TABLE IF NOT EXISTS installed_models (
    id            TEXT PRIMARY KEY,
    filename      TEXT NOT NULL,
    path          TEXT NOT NULL,
    size_bytes    INTEGER NOT NULL,
    sha256        TEXT NOT NULL,
    installed_at  TEXT NOT NULL  -- ISO-8601 UTC
);
