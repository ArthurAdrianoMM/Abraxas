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
    // Fase 5.4: per-conversation generation params. NULL columns mean
    // "fall back to SamplingParams::default() / DEFAULT_COMPLETION_BUDGET".
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<i64>,
    pub repeat_penalty: Option<f64>,
    pub repeat_last_n: Option<i64>,
    pub seed: Option<i64>,
    pub max_completion_tokens: Option<i64>,
}

/// Patch payload for `update_generation_params`. Every field is `Option`:
/// `Some(value)` writes the column, `None` clears it back to NULL (so the
/// resolver falls through to defaults).
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ConversationGenerationParams {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<i64>,
    pub repeat_penalty: Option<f64>,
    pub repeat_last_n: Option<i64>,
    pub seed: Option<i64>,
    pub max_completion_tokens: Option<i64>,
}

const SELECT_COLUMNS: &str = "id, title, model_id, created_at, updated_at, \
                              temperature, top_p, top_k, repeat_penalty, \
                              repeat_last_n, seed, max_completion_tokens";

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
        temperature: None,
        top_p: None,
        top_k: None,
        repeat_penalty: None,
        repeat_last_n: None,
        seed: None,
        max_completion_tokens: None,
    })
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Conversation>, sqlx::Error> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS}
         FROM conversations
         ORDER BY updated_at DESC, created_at DESC"
    );
    sqlx::query_as::<_, Conversation>(&sql)
        .fetch_all(pool)
        .await
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Conversation>, sqlx::Error> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM conversations WHERE id = ?1");
    sqlx::query_as::<_, Conversation>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Overwrites every generation-param column on the row. `Some` writes the
/// value, `None` clears it back to NULL — the resolver treats NULL as
/// "inherit default".
///
/// Returns `true` if a row matched and was updated, `false` if no row
/// existed for `id`.
pub async fn update_generation_params(
    pool: &SqlitePool,
    id: &str,
    params: ConversationGenerationParams,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE conversations
         SET temperature = ?1,
             top_p = ?2,
             top_k = ?3,
             repeat_penalty = ?4,
             repeat_last_n = ?5,
             seed = ?6,
             max_completion_tokens = ?7
         WHERE id = ?8",
    )
    .bind(params.temperature)
    .bind(params.top_p)
    .bind(params.top_k)
    .bind(params.repeat_penalty)
    .bind(params.repeat_last_n)
    .bind(params.seed)
    .bind(params.max_completion_tokens)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Removes every conversation; messages go with them via `ON DELETE CASCADE`.
/// Returns the number of conversations removed.
pub async fn delete_all(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM conversations").execute(pool).await?;
    Ok(result.rows_affected())
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
                id                    TEXT PRIMARY KEY,
                title                 TEXT NOT NULL,
                model_id              TEXT,
                created_at            TEXT NOT NULL,
                updated_at            TEXT NOT NULL,
                temperature           REAL,
                top_p                 REAL,
                top_k                 INTEGER,
                repeat_penalty        REAL,
                repeat_last_n         INTEGER,
                seed                  INTEGER,
                max_completion_tokens INTEGER
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

    #[tokio::test]
    async fn fresh_conversation_has_null_generation_params() {
        let pool = test_pool().await;
        let conv = create(&pool, None, None).await.unwrap();

        assert!(conv.temperature.is_none());
        assert!(conv.top_p.is_none());
        assert!(conv.top_k.is_none());
        assert!(conv.repeat_penalty.is_none());
        assert!(conv.repeat_last_n.is_none());
        assert!(conv.seed.is_none());
        assert!(conv.max_completion_tokens.is_none());

        let fetched = get(&pool, &conv.id).await.unwrap().expect("row exists");
        assert!(fetched.temperature.is_none());
        assert!(fetched.seed.is_none());
    }

    #[tokio::test]
    async fn update_generation_params_round_trips_and_clears() {
        let pool = test_pool().await;
        let conv = create(&pool, None, None).await.unwrap();

        let patch = ConversationGenerationParams {
            temperature: Some(0.1),
            top_p: Some(0.5),
            top_k: Some(20),
            repeat_penalty: Some(1.2),
            repeat_last_n: Some(128),
            seed: Some(42),
            max_completion_tokens: Some(256),
        };
        assert!(update_generation_params(&pool, &conv.id, patch)
            .await
            .unwrap());

        let stored = get(&pool, &conv.id).await.unwrap().expect("row exists");
        assert_eq!(stored.temperature, Some(0.1));
        assert_eq!(stored.top_p, Some(0.5));
        assert_eq!(stored.top_k, Some(20));
        assert_eq!(stored.repeat_penalty, Some(1.2));
        assert_eq!(stored.repeat_last_n, Some(128));
        assert_eq!(stored.seed, Some(42));
        assert_eq!(stored.max_completion_tokens, Some(256));

        // Clearing all back to NULL.
        assert!(update_generation_params(
            &pool,
            &conv.id,
            ConversationGenerationParams::default()
        )
        .await
        .unwrap());
        let cleared = get(&pool, &conv.id).await.unwrap().expect("row exists");
        assert!(cleared.temperature.is_none());
        assert!(cleared.seed.is_none());
    }

    #[tokio::test]
    async fn update_generation_params_on_missing_row_returns_false() {
        let pool = test_pool().await;
        let updated =
            update_generation_params(&pool, "missing", ConversationGenerationParams::default())
                .await
                .unwrap();
        assert!(!updated);
    }
}
