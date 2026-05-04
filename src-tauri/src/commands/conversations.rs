//! Conversation persistence commands for Fase 5.2.

use tauri::State;

use crate::chat::templates::ChatRole;
use crate::db::{conversations, messages, Db};
use crate::error::{AppError, CommandError};

#[tauri::command]
#[specta::specta]
pub async fn create_conversation(
    db: State<'_, Db>,
    title: Option<String>,
    model_id: Option<String>,
) -> Result<conversations::Conversation, CommandError> {
    conversations::create(db.pool(), title, model_id)
        .await
        .map_err(|e| AppError::Db(e).into())
}

#[tauri::command]
#[specta::specta]
pub async fn list_conversations(
    db: State<'_, Db>,
) -> Result<Vec<conversations::Conversation>, CommandError> {
    conversations::list(db.pool())
        .await
        .map_err(|e| AppError::Db(e).into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_conversation(
    db: State<'_, Db>,
    conversation_id: String,
) -> Result<(), CommandError> {
    conversations::delete(db.pool(), &conversation_id)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn append_message(
    db: State<'_, Db>,
    conversation_id: String,
    role: ChatRole,
    content: String,
) -> Result<messages::StoredMessage, CommandError> {
    append_message_to_conversation(db.pool(), &conversation_id, role, content).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_messages(
    db: State<'_, Db>,
    conversation_id: String,
) -> Result<Vec<messages::StoredMessage>, CommandError> {
    list_messages_for_conversation(db.pool(), &conversation_id).await
}

async fn list_messages_for_conversation(
    pool: &sqlx::SqlitePool,
    conversation_id: &str,
) -> Result<Vec<messages::StoredMessage>, CommandError> {
    if !conversations::exists(pool, conversation_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(CommandError {
            kind: "ConversationNotFound".into(),
            message: format!("conversation {conversation_id:?} does not exist"),
        });
    }

    messages::list_for_conversation(pool, conversation_id)
        .await
        .map_err(|e| AppError::Db(e).into())
}

async fn append_message_to_conversation(
    pool: &sqlx::SqlitePool,
    conversation_id: &str,
    role: ChatRole,
    content: String,
) -> Result<messages::StoredMessage, CommandError> {
    if !conversations::exists(pool, conversation_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(CommandError {
            kind: "ConversationNotFound".into(),
            message: format!("conversation {conversation_id:?} does not exist"),
        });
    }

    messages::append(pool, conversation_id, role, content)
        .await
        .map_err(|e| AppError::Db(e).into())
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

    #[tokio::test]
    async fn append_message_persists_rows_reachable_from_list_messages() {
        let pool = test_pool().await;
        let conversation = conversations::create(&pool, Some("Chat".into()), None)
            .await
            .unwrap();

        append_message_to_conversation(&pool, &conversation.id, ChatRole::User, "Hello".into())
            .await
            .unwrap();
        append_message_to_conversation(&pool, &conversation.id, ChatRole::Assistant, "Hi".into())
            .await
            .unwrap();

        let rows = list_messages_for_conversation(&pool, &conversation.id)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].role, ChatRole::User);
        assert_eq!(rows[0].content, "Hello");
        assert_eq!(rows[0].position, 0);
        assert_eq!(rows[1].role, ChatRole::Assistant);
        assert_eq!(rows[1].content, "Hi");
        assert_eq!(rows[1].position, 1);
    }

    #[tokio::test]
    async fn append_message_rejects_missing_conversation() {
        let pool = test_pool().await;

        let err = append_message_to_conversation(&pool, "missing", ChatRole::User, "Hello".into())
            .await
            .unwrap_err();

        assert_eq!(err.kind, "ConversationNotFound");
        assert_eq!(err.message, "conversation \"missing\" does not exist");
    }
}
