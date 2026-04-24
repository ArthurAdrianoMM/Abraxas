//! App-level commands: metadata the frontend needs to render host state.

use crate::error::{AppError, CommandError};
use tauri::{AppHandle, Manager};

#[derive(Debug, serde::Serialize, specta::Type)]
pub struct AppInfo {
    pub version: String,
    pub app_data_dir: String,
    pub log_dir: String,
}

#[tauri::command]
#[specta::specta]
pub async fn app_info(app: AppHandle) -> Result<AppInfo, CommandError> {
    let data_dir = app.path().app_data_dir().map_err(AppError::from)?;
    let log_dir = app.path().app_log_dir().map_err(AppError::from)?;
    let info = AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        app_data_dir: data_dir.to_string_lossy().into_owned(),
        log_dir: log_dir.to_string_lossy().into_owned(),
    };
    tracing::info!(version = %info.version, "app_info invoked");
    Ok(info)
}
