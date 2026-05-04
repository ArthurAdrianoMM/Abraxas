//! `messages` table access.
//!
//! The `role` column is plain TEXT with manual `ChatRole` ↔ string conversion.
//! A custom `sqlx::Type` impl on `ChatRole` would couple `chat::templates` to
//! sqlx; cheaper to translate at the DB boundary for four variants.

use sqlx::{Row, SqlitePool};

use super::now_iso;
use crate::chat::templates::{ChatMessage, ChatRole};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: ChatRole,
    pub content: String,
    pub created_at: String,
}

impl Message {
    pub fn to_chat_message(&self) -> ChatMessage {
        ChatMessage::new(self.role, self.content.clone())
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

fn role_from_str(s: &str) -> Result<ChatRole, sqlx::Error> {
    Ok(match s {
        "system" => ChatRole::System,
        "user" => ChatRole::User,
        "assistant" => ChatRole::Assistant,
        "tool" => ChatRole::Tool,
        other => {
            return Err(sqlx::Error::Protocol(format!(
                "unknown role in messages.role: {other:?}"
            )))
        }
    })
}

fn row_to_message(row: sqlx::sqlite::SqliteRow) -> Result<Message, sqlx::Error> {
    let role: String = row.try_get("role")?;
    Ok(Message {
        id: row.try_get("id")?,
        conversation_id: row.try_get("conversation_id")?,
        role: role_from_str(&role)?,
        content: row.try_get("content")?,
        created_at: row.try_get("created_at")?,
    })
}

/// Inserts a message and bumps the parent conversation's `updated_at` in the
/// same transaction so the conversation list re-sorts atomically.
pub async fn insert(
    pool: &SqlitePool,
    conversation_id: &str,
    role: ChatRole,
    content: &str,
) -> Result<Message, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(&id)
    .bind(conversation_id)
    .bind(role_to_str(role))
    .bind(content)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE conversations SET updated_at = ?1 WHERE id = ?2")
        .bind(&now)
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Message {
        id,
        conversation_id: conversation_id.to_owned(),
        role,
        content: content.to_owned(),
        created_at: now,
    })
}

pub async fn list_for_conversation(
    pool: &SqlitePool,
    conversation_id: &str,
) -> Result<Vec<Message>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, conversation_id, role, content, created_at \
         FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC, id ASC",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(row_to_message).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{conversations, Db};
    use tempfile::tempdir;

    async fn fresh_db() -> (tempfile::TempDir, Db) {
        let dir = tempdir().unwrap();
        let db = Db::init(&dir.path().join("test.sqlite")).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn insert_and_list_in_order() {
        let (_g, db) = fresh_db().await;
        let c = conversations::insert(db.pool(), "t").await.unwrap();
        let m1 = insert(db.pool(), &c.id, ChatRole::User, "hi").await.unwrap();
        // Tiny gap so created_at differs.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let m2 = insert(db.pool(), &c.id, ChatRole::Assistant, "yo")
            .await
            .unwrap();

        let rows = list_for_conversation(db.pool(), &c.id).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, m1.id);
        assert_eq!(rows[0].role, ChatRole::User);
        assert_eq!(rows[1].id, m2.id);
        assert_eq!(rows[1].role, ChatRole::Assistant);
    }

    #[tokio::test]
    async fn insert_bumps_conversation_updated_at() {
        let (_g, db) = fresh_db().await;
        let c = conversations::insert(db.pool(), "t").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        insert(db.pool(), &c.id, ChatRole::User, "hi").await.unwrap();

        let rows = conversations::list(db.pool()).await.unwrap();
        assert!(
            rows[0].updated_at > c.updated_at,
            "updated_at should advance after a message insert (was {}, now {})",
            c.updated_at,
            rows[0].updated_at
        );
    }

    #[tokio::test]
    async fn delete_conversation_cascades_messages() {
        let (_g, db) = fresh_db().await;
        let c = conversations::insert(db.pool(), "t").await.unwrap();
        insert(db.pool(), &c.id, ChatRole::User, "hi").await.unwrap();
        insert(db.pool(), &c.id, ChatRole::Assistant, "yo")
            .await
            .unwrap();

        assert!(conversations::delete(db.pool(), &c.id).await.unwrap());
        let rows = list_for_conversation(db.pool(), &c.id).await.unwrap();
        assert!(rows.is_empty(), "messages should cascade on conversation delete");
    }

    #[tokio::test]
    async fn role_round_trips_all_variants() {
        let (_g, db) = fresh_db().await;
        let c = conversations::insert(db.pool(), "t").await.unwrap();
        for role in [
            ChatRole::System,
            ChatRole::User,
            ChatRole::Assistant,
            ChatRole::Tool,
        ] {
            insert(db.pool(), &c.id, role, "x").await.unwrap();
            // Tiny gap so created_at strictly increases between inserts.
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        let rows = list_for_conversation(db.pool(), &c.id).await.unwrap();
        let roles: Vec<_> = rows.iter().map(|m| m.role).collect();
        assert_eq!(
            roles,
            vec![
                ChatRole::System,
                ChatRole::User,
                ChatRole::Assistant,
                ChatRole::Tool,
            ]
        );
    }
}
