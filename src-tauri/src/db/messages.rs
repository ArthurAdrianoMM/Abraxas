//! `messages` table access.
//!
//! This module owns message ordering within a conversation. Callers pass the
//! role/content; the next `position` and parent `updated_at` are committed in
//! one transaction.

use sqlx::SqlitePool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::chat::templates::ChatRole;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
pub struct StoredMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: ChatRole,
    pub content: String,
    pub position: i64,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    position: i64,
    created_at: String,
}

pub async fn append(
    pool: &SqlitePool,
    conversation_id: &str,
    role: ChatRole,
    content: String,
) -> Result<StoredMessage, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let now = now_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();
    let position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM messages WHERE conversation_id = ?1",
    )
    .bind(conversation_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, position, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(conversation_id)
    .bind(role_to_str(role))
    .bind(&content)
    .bind(position)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE conversations SET updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(StoredMessage {
        id,
        conversation_id: conversation_id.to_owned(),
        role,
        content,
        position,
        created_at: now,
    })
}

pub async fn list_for_conversation(
    pool: &SqlitePool,
    conversation_id: &str,
) -> Result<Vec<StoredMessage>, sqlx::Error> {
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT id, conversation_id, role, content, position, created_at
         FROM messages
         WHERE conversation_id = ?1
         ORDER BY position ASC",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(StoredMessage::try_from).collect()
}

impl TryFrom<MessageRow> for StoredMessage {
    type Error = sqlx::Error;

    fn try_from(row: MessageRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            role: role_from_str(&row.role)?,
            content: row.content,
            position: row.position,
            created_at: row.created_at,
        })
    }
}

fn role_to_str(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::Tool => "tool",
    }
}

fn role_from_str(role: &str) -> Result<ChatRole, sqlx::Error> {
    match role {
        "system" => Ok(ChatRole::System),
        "user" => Ok(ChatRole::User),
        "assistant" => Ok(ChatRole::Assistant),
        "tool" => Ok(ChatRole::Tool),
        _ => Err(sqlx::Error::ColumnDecode {
            index: "role".into(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid chat role {role:?}"),
            )
            .into(),
        }),
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::templates::ChatRole;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let pool =
            SqlitePool::connect_with(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                .await
                .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
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
        sqlx::query(
            "CREATE TABLE messages (
                id              TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                role            TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
                content         TEXT NOT NULL,
                position        INTEGER NOT NULL,
                created_at      TEXT NOT NULL,
                UNIQUE(conversation_id, position)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    async fn insert_conversation(pool: &SqlitePool, id: &str, updated_at: &str) {
        sqlx::query(
            "INSERT INTO conversations (id, title, model_id, created_at, updated_at)
             VALUES (?1, 'Chat', NULL, ?2, ?2)",
        )
        .bind(id)
        .bind(updated_at)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn append_assigns_positions_and_lists_in_order() {
        let pool = test_pool().await;
        insert_conversation(&pool, "c1", "2026-05-04T00:00:00Z").await;

        let user = append(&pool, "c1", ChatRole::User, "Hello".into())
            .await
            .unwrap();
        let assistant = append(&pool, "c1", ChatRole::Assistant, "Hi".into())
            .await
            .unwrap();

        assert_eq!(user.position, 0);
        assert_eq!(assistant.position, 1);

        let rows = list_for_conversation(&pool, "c1").await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].role, ChatRole::User);
        assert_eq!(rows[0].content, "Hello");
        assert_eq!(rows[1].role, ChatRole::Assistant);
        assert_eq!(rows[1].content, "Hi");
    }

    #[tokio::test]
    async fn append_updates_conversation_timestamp() {
        let pool = test_pool().await;
        insert_conversation(&pool, "c1", "2026-05-04T00:00:00Z").await;

        append(&pool, "c1", ChatRole::System, "Stay concise".into())
            .await
            .unwrap();

        let updated_at: String =
            sqlx::query_scalar("SELECT updated_at FROM conversations WHERE id = 'c1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_ne!(updated_at, "2026-05-04T00:00:00Z");
    }

    #[tokio::test]
    async fn deleting_conversation_cascades_messages() {
        let pool = test_pool().await;
        insert_conversation(&pool, "c1", "2026-05-04T00:00:00Z").await;
        append(&pool, "c1", ChatRole::Tool, "result".into())
            .await
            .unwrap();

        sqlx::query("DELETE FROM conversations WHERE id = 'c1'")
            .execute(&pool)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
