//! `conversations` table access.
//!
//! This module stays deliberately thin: SQL and row shaping only. Command
//! modules own frontend-facing orchestration.

use sqlx::SqlitePool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const DEFAULT_TITLE: &str = "Nova conversa";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, sqlx::FromRow)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn create(
    pool: &SqlitePool,
    title: Option<String>,
    model_id: Option<String>,
) -> Result<Conversation, sqlx::Error> {
    let now = now_rfc3339();
    let title = normalize_title(title);
    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO conversations (id, title, model_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(&id)
    .bind(&title)
    .bind(&model_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(Conversation {
        id,
        title,
        model_id,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Conversation>, sqlx::Error> {
    sqlx::query_as::<_, Conversation>(
        "SELECT id, title, model_id, created_at, updated_at
         FROM conversations
         ORDER BY updated_at DESC, created_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Returns `true` if a row for `id` existed and was removed.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM conversations WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn exists(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE id = ?1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(count > 0)
}

fn normalize_title(title: Option<String>) -> String {
    title
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| DEFAULT_TITLE.to_owned())
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let pool =
            SqlitePool::connect_with(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                .await
                .unwrap();
        sqlx::query(
            "CREATE TABLE conversations (
                id         TEXT PRIMARY KEY,
                title      TEXT NOT NULL,
                model_id   TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn create_defaults_blank_title_and_list_orders_newest_first() {
        let pool = test_pool().await;

        let first = create(&pool, Some("   ".into()), None).await.unwrap();
        let second = create(&pool, Some("Research".into()), Some("tinyllama".into()))
            .await
            .unwrap();

        assert_eq!(first.title, "Nova conversa");
        assert_eq!(second.title, "Research");
        assert_eq!(second.model_id.as_deref(), Some("tinyllama"));

        let rows = list(&pool).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, second.id);
        assert_eq!(rows[1].id, first.id);
    }

    #[tokio::test]
    async fn delete_is_idempotent_and_exists_reflects_state() {
        let pool = test_pool().await;
        let conversation = create(&pool, Some("Chat".into()), None).await.unwrap();

        assert!(exists(&pool, &conversation.id).await.unwrap());
        assert!(delete(&pool, &conversation.id).await.unwrap());
        assert!(!exists(&pool, &conversation.id).await.unwrap());
        assert!(!delete(&pool, &conversation.id).await.unwrap());
    }
}
