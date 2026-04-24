//! App-wide error types. Grows through later phases.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("failed to resolve app log directory: {0}")]
    LogDir(#[from] tauri::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("tracing subscriber already initialized: {0}")]
    TracingInit(#[from] tracing_subscriber::util::TryInitError),

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}
