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

    #[error("inference error: {0}")]
    Inference(#[from] crate::inference::InferenceError),

    #[error("catalog error: {0}")]
    Catalog(#[from] crate::models::catalog::CatalogError),
}

/// Frontend-facing error shape. Stable across `AppError` refactors so the TS
/// binding stays backwards-compatible.
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct CommandError {
    pub kind: String,
    pub message: String,
}

impl From<AppError> for CommandError {
    fn from(e: AppError) -> Self {
        let kind = match &e {
            AppError::LogDir(_) => "LogDir",
            AppError::Io(_) => "Io",
            AppError::TracingInit(_) => "TracingInit",
            AppError::Db(_) => "Db",
            AppError::Migrate(_) => "Migrate",
            AppError::Inference(_) => "Inference",
            AppError::Catalog(_) => "Catalog",
        };
        Self {
            kind: kind.to_owned(),
            message: e.to_string(),
        }
    }
}
