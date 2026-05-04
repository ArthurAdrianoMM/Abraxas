//! `conversations` table access.
//!
//! Thin SQL layer — orchestration (events, validation, default titles) lives in
//! the commands layer. Mirrors `models::registry`'s "no business logic here"
//! style.

use sqlx::SqlitePool;

use super::now_iso;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, sqlx::FromRow)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn insert(pool: &SqlitePool, title: &str) -> Result<Conversation, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();
    sqlx::query(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
    )
    .bind(&id)
    .bind(title)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(Conversation {
        id,
        title: title.to_owned(),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Conversation>, sqlx::Error> {
    sqlx::query_as::<_, Conversation>(
        "SELECT id, title, created_at, updated_at \
         FROM conversations ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Returns `true` if a row for `id` existed and was removed. Cascades to
/// `messages` rows via the FK declared in `0003_conversations.sql`.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM conversations WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use tempfile::tempdir;

    async fn fresh_db() -> (tempfile::TempDir, Db) {
        let dir = tempdir().unwrap();
        let db = Db::init(&dir.path().join("test.sqlite")).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn insert_then_list_returns_row() {
        let (_g, db) = fresh_db().await;
        let c = insert(db.pool(), "hello").await.unwrap();
        assert_eq!(c.title, "hello");
        let rows = list(db.pool()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, c.id);
    }

    #[tokio::test]
    async fn list_orders_by_updated_at_desc() {
        let (_g, db) = fresh_db().await;
        let a = insert(db.pool(), "a").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let b = insert(db.pool(), "b").await.unwrap();
        let rows = list(db.pool()).await.unwrap();
        assert_eq!(rows[0].id, b.id, "newer conversation is first");
        assert_eq!(rows[1].id, a.id);
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let (_g, db) = fresh_db().await;
        let c = insert(db.pool(), "x").await.unwrap();
        assert!(delete(db.pool(), &c.id).await.unwrap());
        assert!(list(db.pool()).await.unwrap().is_empty());
        assert!(!delete(db.pool(), &c.id).await.unwrap());
    }
}
