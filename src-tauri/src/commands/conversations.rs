//! Conversation & message commands.
//!
//! Thin wrappers over the `db::conversations` and `db::messages` modules.

use tauri::State;

use crate::db::conversations::Conversation;
use crate::db::messages::Message;
use crate::db::{conversations, messages, Db};
use crate::error::CommandError;

const DEFAULT_TITLE: &str = "New conversation";

#[tauri::command]
#[specta::specta]
pub async fn create_conversation(
    db: State<'_, Db>,
    title: Option<String>,
) -> Result<Conversation, CommandError> {
    let title = title
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| DEFAULT_TITLE.to_owned());
    Ok(conversations::insert(db.pool(), &title).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_conversations(db: State<'_, Db>) -> Result<Vec<Conversation>, CommandError> {
    Ok(conversations::list(db.pool()).await?)
}

/// Idempotent: deleting a non-existent id succeeds silently.
#[tauri::command]
#[specta::specta]
pub async fn delete_conversation(db: State<'_, Db>, id: String) -> Result<(), CommandError> {
    conversations::delete(db.pool(), &id).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn list_messages(
    db: State<'_, Db>,
    conversation_id: String,
) -> Result<Vec<Message>, CommandError> {
    Ok(messages::list_for_conversation(db.pool(), &conversation_id).await?)
}
