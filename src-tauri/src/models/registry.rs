//! Installed-models registry backed by the `installed_models` SQLite table.
//!
//! This module is deliberately thin: only SQL, no business logic. Callers
//! (commands layer) own orchestration — file deletion, event emission, etc.
//!
//! Dynamic query variants (no `!` suffix) are used throughout so this module
//! compiles without DATABASE_URL or a prepared sqlx cache.
//!
//! Since migration 0006 a row is self-describing: it carries where the model
//! came from (`source`), how to render prompts for it (`chat_template`) and
//! its context window, so loading a model never has to consult the catalog.
//! Catalog rows written before that migration have `chat_template = NULL`
//! and are resolved through the catalog cache at load time.

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

/// Where an installed model came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    /// Downloaded from the remote catalog; checksum-verified.
    Catalog,
    /// A GGUF the user already had on disk, referenced in place.
    Local,
    /// Downloaded from a URL the user supplied; not verified.
    Url,
}

impl ModelSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ModelSource::Catalog => "catalog",
            ModelSource::Local => "local",
            ModelSource::Url => "url",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "local" => ModelSource::Local,
            "url" => ModelSource::Url,
            _ => ModelSource::Catalog,
        }
    }
}

/// How prompts are rendered for an installed model.
///
/// Stored as text in `installed_models.chat_template`: a `ChatTemplate`
/// family name, or `"Embedded"` for the GGUF's own `tokenizer.chat_template`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(tag = "kind", content = "family", rename_all = "snake_case")]
pub enum InstalledTemplate {
    /// One of the hand-written family renderers in `chat::templates`.
    Family(crate::models::catalog::ChatTemplate),
    /// The Jinja template baked into the GGUF, rendered by llama.cpp.
    Embedded,
}

impl InstalledTemplate {
    fn to_column(self) -> String {
        match self {
            InstalledTemplate::Embedded => "Embedded".to_owned(),
            // `ChatTemplate` serializes as its bare variant name.
            InstalledTemplate::Family(f) => serde_json::to_value(f)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
        }
    }

    fn from_column(s: &str) -> Option<Self> {
        if s == "Embedded" {
            return Some(InstalledTemplate::Embedded);
        }
        serde_json::from_value(serde_json::Value::String(s.to_owned()))
            .ok()
            .map(InstalledTemplate::Family)
    }
}

/// One row in the `installed_models` table. Returned directly to the frontend
/// via Tauri commands, so it must be serializable and have a specta type.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstalledModel {
    pub id: String,
    pub filename: String,
    pub path: String,
    pub size_bytes: i64,
    /// `None` for user-supplied models: nothing to verify them against.
    pub sha256: Option<String>,
    pub installed_at: String,
    pub source: ModelSource,
    /// The URL a `Url` model was fetched from (the HF page or direct link).
    pub source_url: Option<String>,
    /// Human name. Catalog rows leave this `None` and the frontend uses the
    /// catalog entry; custom rows fill it from the GGUF's `general.name`.
    pub display_name: Option<String>,
    /// `None` = not decided yet. For a custom model this means the GGUF had
    /// no usable embedded template and the user has to pick a family before
    /// the model can chat.
    pub chat_template: Option<InstalledTemplate>,
    pub context_length: Option<u32>,
}

/// Raw row shape; converted into `InstalledModel` so the enum columns stay
/// plain TEXT in SQLite.
#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    filename: String,
    path: String,
    size_bytes: i64,
    sha256: Option<String>,
    installed_at: String,
    source: String,
    source_url: Option<String>,
    display_name: Option<String>,
    chat_template: Option<String>,
    context_length: Option<i64>,
}

impl From<Row> for InstalledModel {
    fn from(r: Row) -> Self {
        InstalledModel {
            id: r.id,
            filename: r.filename,
            path: r.path,
            size_bytes: r.size_bytes,
            sha256: r.sha256,
            installed_at: r.installed_at,
            source: ModelSource::parse(&r.source),
            source_url: r.source_url,
            display_name: r.display_name,
            chat_template: r
                .chat_template
                .as_deref()
                .and_then(InstalledTemplate::from_column),
            context_length: r.context_length.and_then(|n| u32::try_from(n).ok()),
        }
    }
}

const COLUMNS: &str = "id, filename, path, size_bytes, sha256, installed_at, \
                       source, source_url, display_name, chat_template, context_length";

pub async fn insert(pool: &SqlitePool, model: &InstalledModel) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO installed_models
            (id, filename, path, size_bytes, sha256, installed_at,
             source, source_url, display_name, chat_template, context_length)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(id) DO UPDATE SET
            filename       = excluded.filename,
            path           = excluded.path,
            size_bytes     = excluded.size_bytes,
            sha256         = excluded.sha256,
            installed_at   = excluded.installed_at,
            source         = excluded.source,
            source_url     = excluded.source_url,
            display_name   = excluded.display_name,
            chat_template  = excluded.chat_template,
            context_length = excluded.context_length
        "#,
    )
    .bind(&model.id)
    .bind(&model.filename)
    .bind(&model.path)
    .bind(model.size_bytes)
    .bind(&model.sha256)
    .bind(&model.installed_at)
    .bind(model.source.as_str())
    .bind(&model.source_url)
    .bind(&model.display_name)
    .bind(model.chat_template.map(InstalledTemplate::to_column))
    .bind(model.context_length.map(i64::from))
    .execute(pool)
    .await?;
    Ok(())
}

/// Set (or clear) the chat template of an existing row. Used by the escape
/// hatch for custom models whose GGUF carries no usable template.
pub async fn set_chat_template(
    pool: &SqlitePool,
    id: &str,
    template: Option<InstalledTemplate>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE installed_models SET chat_template = ?2 WHERE id = ?1")
        .bind(id)
        .bind(template.map(InstalledTemplate::to_column))
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Returns `true` if a row for `id` existed and was removed.
pub async fn remove(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM installed_models WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<InstalledModel>, sqlx::Error> {
    let rows = sqlx::query_as::<_, Row>(&format!(
        "SELECT {COLUMNS} FROM installed_models ORDER BY installed_at DESC"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(InstalledModel::from).collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<InstalledModel>, sqlx::Error> {
    let row = sqlx::query_as::<_, Row>(&format!(
        "SELECT {COLUMNS} FROM installed_models WHERE id = ?1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(InstalledModel::from))
}

/// Look a row up by its on-disk path. Two rows must never share a file:
/// deleting one would silently break the other.
pub async fn get_by_path(
    pool: &SqlitePool,
    path: &str,
) -> Result<Option<InstalledModel>, sqlx::Error> {
    let row = sqlx::query_as::<_, Row>(&format!(
        "SELECT {COLUMNS} FROM installed_models WHERE path = ?1"
    ))
    .bind(path)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(InstalledModel::from))
}

pub async fn exists(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM installed_models WHERE id = ?1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(count > 0)
}

/// Removes rows whose `path` no longer exists on disk. Returns count removed.
/// Called once at app startup to self-heal after external file deletion.
///
/// Uses `tokio::fs::try_exists` so the async runtime is not blocked.
/// On I/O error (e.g. permission denied), the row is conservatively retained —
/// a transient access failure is not proof the file is gone.
pub async fn reconcile(pool: &SqlitePool) -> Result<u32, sqlx::Error> {
    let rows = list(pool).await?;
    let mut removed = 0u32;
    for row in rows {
        let exists = tokio::fs::try_exists(&row.path).await.unwrap_or(true);
        if !exists {
            tracing::info!(
                model_id = %row.id,
                path = %row.path,
                "reconcile: installed_models row removed (file missing)"
            );
            remove(pool, &row.id).await?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::catalog::ChatTemplate;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let pool =
            SqlitePool::connect_with(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                .await
                .unwrap();
        sqlx::query(
            "CREATE TABLE installed_models (
                id              TEXT PRIMARY KEY,
                filename        TEXT NOT NULL,
                path            TEXT NOT NULL,
                size_bytes      INTEGER NOT NULL,
                sha256          TEXT,
                installed_at    TEXT NOT NULL,
                source          TEXT NOT NULL DEFAULT 'catalog',
                source_url      TEXT,
                display_name    TEXT,
                chat_template   TEXT,
                context_length  INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn sample(id: &str) -> InstalledModel {
        InstalledModel {
            id: id.to_owned(),
            filename: format!("{id}.gguf"),
            path: format!("/models/{id}.gguf"),
            size_bytes: 1_000_000,
            sha256: Some("abc123".to_owned()),
            installed_at: "2026-04-30T00:00:00Z".to_owned(),
            source: ModelSource::Catalog,
            source_url: None,
            display_name: None,
            chat_template: None,
            context_length: None,
        }
    }

    #[tokio::test]
    async fn insert_and_list() {
        let pool = test_pool().await;
        insert(&pool, &sample("m1")).await.unwrap();
        insert(&pool, &sample("m2")).await.unwrap();
        let models = list(&pool).await.unwrap();
        assert_eq!(models.len(), 2);
    }

    #[tokio::test]
    async fn insert_upserts_on_conflict() {
        let pool = test_pool().await;
        insert(&pool, &sample("m1")).await.unwrap();
        let mut updated = sample("m1");
        updated.size_bytes = 9_999_999;
        insert(&pool, &updated).await.unwrap();
        let row = get(&pool, "m1").await.unwrap().unwrap();
        assert_eq!(row.size_bytes, 9_999_999);
        assert_eq!(list(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn custom_row_roundtrips_every_column() {
        let pool = test_pool().await;
        let row = InstalledModel {
            sha256: None,
            source: ModelSource::Url,
            source_url: Some("https://huggingface.co/x/y".into()),
            display_name: Some("My Model".into()),
            chat_template: Some(InstalledTemplate::Embedded),
            context_length: Some(8192),
            ..sample("custom")
        };
        insert(&pool, &row).await.unwrap();
        let back = get(&pool, "custom").await.unwrap().unwrap();
        assert_eq!(back.sha256, None);
        assert_eq!(back.source, ModelSource::Url);
        assert_eq!(
            back.source_url.as_deref(),
            Some("https://huggingface.co/x/y")
        );
        assert_eq!(back.display_name.as_deref(), Some("My Model"));
        assert_eq!(back.chat_template, Some(InstalledTemplate::Embedded));
        assert_eq!(back.context_length, Some(8192));
    }

    #[tokio::test]
    async fn family_template_roundtrips_by_name() {
        let pool = test_pool().await;
        let row = InstalledModel {
            chat_template: Some(InstalledTemplate::Family(ChatTemplate::Llama3)),
            ..sample("fam")
        };
        insert(&pool, &row).await.unwrap();
        let stored: String =
            sqlx::query_scalar("SELECT chat_template FROM installed_models WHERE id = 'fam'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, "Llama3");
        let back = get(&pool, "fam").await.unwrap().unwrap();
        assert_eq!(
            back.chat_template,
            Some(InstalledTemplate::Family(ChatTemplate::Llama3))
        );
    }

    #[tokio::test]
    async fn unknown_template_column_reads_as_none() {
        let pool = test_pool().await;
        insert(&pool, &sample("m1")).await.unwrap();
        sqlx::query("UPDATE installed_models SET chat_template = 'NotAFamily' WHERE id = 'm1'")
            .execute(&pool)
            .await
            .unwrap();
        let back = get(&pool, "m1").await.unwrap().unwrap();
        assert_eq!(back.chat_template, None);
    }

    #[tokio::test]
    async fn set_chat_template_updates_existing_row() {
        let pool = test_pool().await;
        insert(&pool, &sample("m1")).await.unwrap();
        let changed = set_chat_template(
            &pool,
            "m1",
            Some(InstalledTemplate::Family(ChatTemplate::ChatML)),
        )
        .await
        .unwrap();
        assert!(changed);
        let back = get(&pool, "m1").await.unwrap().unwrap();
        assert_eq!(
            back.chat_template,
            Some(InstalledTemplate::Family(ChatTemplate::ChatML))
        );
        assert!(!set_chat_template(&pool, "nope", None).await.unwrap());
    }

    #[tokio::test]
    async fn get_by_path_finds_row() {
        let pool = test_pool().await;
        insert(&pool, &sample("m1")).await.unwrap();
        let found = get_by_path(&pool, "/models/m1.gguf").await.unwrap();
        assert_eq!(found.map(|r| r.id).as_deref(), Some("m1"));
        assert!(get_by_path(&pool, "/models/none.gguf")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn remove_returns_true_for_existing_row() {
        let pool = test_pool().await;
        insert(&pool, &sample("m1")).await.unwrap();
        assert!(remove(&pool, "m1").await.unwrap());
        assert_eq!(list(&pool).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn remove_returns_false_for_missing_row() {
        let pool = test_pool().await;
        assert!(!remove(&pool, "nope").await.unwrap());
    }

    #[tokio::test]
    async fn exists_correct() {
        let pool = test_pool().await;
        assert!(!exists(&pool, "m1").await.unwrap());
        insert(&pool, &sample("m1")).await.unwrap();
        assert!(exists(&pool, "m1").await.unwrap());
    }

    #[tokio::test]
    async fn get_returns_none_for_missing() {
        let pool = test_pool().await;
        assert!(get(&pool, "nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn reconcile_removes_rows_with_missing_files() {
        let pool = test_pool().await;
        // real temp file — exists on disk
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut real = sample("real");
        real.path = tmp.path().to_string_lossy().into_owned();
        insert(&pool, &real).await.unwrap();

        // ghost path — does not exist
        let mut ghost = sample("ghost");
        ghost.path = "/nonexistent/path/model.gguf".to_owned();
        insert(&pool, &ghost).await.unwrap();

        let removed = reconcile(&pool).await.unwrap();
        assert_eq!(removed, 1);
        assert!(exists(&pool, "real").await.unwrap());
        assert!(!exists(&pool, "ghost").await.unwrap());
    }
}
