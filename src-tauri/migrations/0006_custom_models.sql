-- Custom models: a model no longer has to come from the catalog. It can be a
-- GGUF the user already had on disk (`source = 'local'`) or one downloaded
-- from a URL the user pasted (`source = 'url'`). Those rows carry their own
-- chat template and context length, so loading no longer depends on a
-- catalog lookup; catalog rows written before this migration keep NULL here
-- and fall back to the catalog cache at load time.
--
-- `sha256` becomes nullable: user-supplied models are not verified (there is
-- no published checksum to compare against). SQLite cannot drop NOT NULL in
-- place, so the table is rebuilt and the existing rows copied over.

CREATE TABLE installed_models_v2 (
    id              TEXT PRIMARY KEY,
    filename        TEXT NOT NULL,
    path            TEXT NOT NULL,
    size_bytes      INTEGER NOT NULL,
    sha256          TEXT,
    installed_at    TEXT NOT NULL,  -- ISO-8601 UTC
    source          TEXT NOT NULL DEFAULT 'catalog'
                    CHECK(source IN ('catalog', 'local', 'url')),
    source_url      TEXT,
    display_name    TEXT,
    -- Chat-template family name (see `ChatTemplate`), or 'Embedded' when the
    -- GGUF's own `tokenizer.chat_template` is used. NULL = unknown yet.
    chat_template   TEXT,
    context_length  INTEGER
);

INSERT INTO installed_models_v2 (id, filename, path, size_bytes, sha256, installed_at)
    SELECT id, filename, path, size_bytes, sha256, installed_at FROM installed_models;

DROP TABLE installed_models;
ALTER TABLE installed_models_v2 RENAME TO installed_models;
