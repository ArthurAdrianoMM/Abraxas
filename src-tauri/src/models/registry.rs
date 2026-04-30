//! Installed-models registry backed by the `installed_models` SQLite table.
//!
//! This module is deliberately thin: only SQL, no business logic. Callers
//! (commands layer) own orchestration — file deletion, event emission, etc.
//!
//! Dynamic query variants (no `!` suffix) are used throughout so this module
//! compiles without DATABASE_URL or a prepared sqlx cache.

use sqlx::SqlitePool;

/// One row in the `installed_models` table. Returned directly to the frontend
/// via Tauri commands, so it must be serializable and have a specta type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, sqlx::FromRow)]
pub struct InstalledModel {
    pub id: String,
    pub filename: String,
    pub path: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub installed_at: String,
}

pub async fn insert(pool: &SqlitePool, model: &InstalledModel) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO installed_models (id, filename, path, size_bytes, sha256, installed_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(id) DO UPDATE SET
            filename     = excluded.filename,
            path         = excluded.path,
            size_bytes   = excluded.size_bytes,
            sha256       = excluded.sha256,
            installed_at = excluded.installed_at
        "#,
    )
    .bind(&model.id)
    .bind(&model.filename)
    .bind(&model.path)
    .bind(model.size_bytes)
    .bind(&model.sha256)
    .bind(&model.installed_at)
    .execute(pool)
    .await?;
    Ok(())
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
    sqlx::query_as::<_, InstalledModel>(
        "SELECT id, filename, path, size_bytes, sha256, installed_at \
         FROM installed_models ORDER BY installed_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<InstalledModel>, sqlx::Error> {
    sqlx::query_as::<_, InstalledModel>(
        "SELECT id, filename, path, size_bytes, sha256, installed_at \
         FROM installed_models WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn exists(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM installed_models WHERE id = ?1")
            .bind(id)
            .fetch_one(pool)
            .await?;
    Ok(count > 0)
}

/// Removes rows whose `path` no longer exists on disk. Returns count removed.
/// Called once at app startup to self-heal after external file deletion.
pub async fn reconcile(pool: &SqlitePool) -> Result<u32, sqlx::Error> {
    let rows = list(pool).await?;
    let mut removed = 0u32;
    for row in rows {
        if !std::path::Path::new(&row.path).exists() {
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
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect_with(
            SqliteConnectOptions::from_str("sqlite::memory:").unwrap(),
        )
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE installed_models (
                id            TEXT PRIMARY KEY,
                filename      TEXT NOT NULL,
                path          TEXT NOT NULL,
                size_bytes    INTEGER NOT NULL,
                sha256        TEXT NOT NULL,
                installed_at  TEXT NOT NULL
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
            sha256: "abc123".to_owned(),
            installed_at: "2026-04-30T00:00:00Z".to_owned(),
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
