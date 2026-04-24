//! SQLite persistence via `sqlx`.
//!
//! `Db::init` is called once at app startup (see `lib.rs`) with a path under
//! the per-OS app data directory. It creates the file if missing, configures
//! WAL + NORMAL sync + foreign keys, and runs all embedded migrations.

pub mod app_settings;
pub mod conversations;
pub mod messages;

use std::path::Path;
use std::time::Duration;

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

use crate::error::AppError;

/// Wraps the pooled connection to the app's SQLite database.
///
/// Held in Tauri state from startup. The field and `pool()` accessor become
/// live consumers once Phase 1.5 wires typed commands.
pub struct Db(#[allow(dead_code)] SqlitePool);

#[allow(dead_code)]
impl Db {
    /// Open (creating if missing) the database at `db_path` and apply any
    /// pending migrations. The parent directory is created if it doesn't exist.
    pub async fn init(db_path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect_with(options)
            .await?;

        let migrator = sqlx::migrate!("./migrations");
        let total = migrator.iter().count();
        migrator.run(&pool).await?;
        tracing::info!(total, "db migrations applied");

        Ok(Self(pool))
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn init_runs_migrations_and_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db_path = tmp.path().join("abraxas.sqlite");

        let db = Db::init(&db_path).await.expect("first Db::init failed");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(db.pool())
            .await
            .expect("count _sqlx_migrations");
        assert_eq!(count, 1, "first init should apply exactly 1 migration");
        drop(db);

        let db2 = Db::init(&db_path).await.expect("second Db::init failed");
        let count_again: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(db2.pool())
            .await
            .expect("count _sqlx_migrations on second init");
        assert_eq!(
            count_again, 1,
            "re-opening an existing db must not re-apply migrations"
        );
    }
}
