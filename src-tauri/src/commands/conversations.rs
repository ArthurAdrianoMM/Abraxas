//! Conversation persistence commands for Fase 5.2.

use tauri::State;

use crate::chat::templates::ChatRole;
use crate::db::{app_settings, conversations, messages, Db};
use crate::error::{AppError, CommandError};

/// Creates the conversation and stamps the app-wide default generation
/// params onto it (Fase 6.2). Stamping at creation time — instead of
/// resolving defaults at generation time — keeps existing conversations
/// untouched when the user later changes a default.
#[tauri::command]
#[specta::specta]
pub async fn create_conversation(
    db: State<'_, Db>,
    title: Option<String>,
    model_id: Option<String>,
) -> Result<conversations::Conversation, CommandError> {
    create_conversation_with_defaults(db.pool(), title, model_id).await
}

async fn create_conversation_with_defaults(
    pool: &sqlx::SqlitePool,
    title: Option<String>,
    model_id: Option<String>,
) -> Result<conversations::Conversation, CommandError> {
    let settings = app_settings::get(pool).await.map_err(AppError::Db)?;
    let params = conversations::ConversationGenerationParams {
        temperature: Some(settings.default_temperature),
        top_p: Some(settings.default_top_p),
        max_completion_tokens: Some(settings.default_max_completion_tokens as i64),
        seed: settings.default_seed,
        ..Default::default()
    };

    let mut conversation = conversations::create(pool, title, model_id)
        .await
        .map_err(AppError::Db)?;
    conversations::update_generation_params(pool, &conversation.id, params)
        .await
        .map_err(AppError::Db)?;

    conversation.temperature = params.temperature;
    conversation.top_p = params.top_p;
    conversation.max_completion_tokens = params.max_completion_tokens;
    conversation.seed = params.seed;
    Ok(conversation)
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
pub async fn update_conversation_generation_params(
    db: State<'_, Db>,
    conversation_id: String,
    params: conversations::ConversationGenerationParams,
) -> Result<conversations::Conversation, CommandError> {
    let updated = conversations::update_generation_params(db.pool(), &conversation_id, params)
        .await
        .map_err(AppError::Db)?;
    if !updated {
        return Err(CommandError {
            kind: "ConversationNotFound".into(),
            message: format!("conversation {conversation_id:?} does not exist"),
        });
    }
    conversations::get(db.pool(), &conversation_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| CommandError {
            kind: "ConversationNotFound".into(),
            message: format!("conversation {conversation_id:?} disappeared after update"),
        })
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
        sqlx::query(
            "CREATE TABLE app_settings (
                key   TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn create_conversation_stamps_current_defaults_and_leaves_old_rows() {
        let pool = test_pool().await;

        // With no stored settings, the built-in defaults get stamped.
        let first = create_conversation_with_defaults(&pool, None, None)
            .await
            .unwrap();
        assert_eq!(first.temperature, Some(0.8));
        assert_eq!(first.top_p, Some(0.95));
        assert_eq!(first.max_completion_tokens, Some(512));
        assert_eq!(first.seed, None);

        // Change the defaults; only conversations created afterwards see them.
        let settings = crate::db::app_settings::AppSettings {
            default_temperature: 1.2,
            default_seed: Some(42),
            ..Default::default()
        };
        crate::db::app_settings::set(&pool, &settings).await.unwrap();

        let second = create_conversation_with_defaults(&pool, None, None)
            .await
            .unwrap();
        assert_eq!(second.temperature, Some(1.2));
        assert_eq!(second.seed, Some(42));

        let first_again = conversations::get(&pool, &first.id).await.unwrap().unwrap();
        assert_eq!(first_again.temperature, Some(0.8));
        assert_eq!(first_again.seed, None);
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
