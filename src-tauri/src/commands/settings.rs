//! Settings commands (Fase 6.2): app preferences, disk usage, model-file
//! integrity, and the destructive "clear data" actions.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::db::app_settings::{self, AppSettings, IntegrityCheck};
use crate::db::{conversations, Db};
use crate::error::{AppError, CommandError};
use crate::inference::ModelManager;
use crate::models::registry;

#[tauri::command]
#[specta::specta]
pub async fn get_app_settings(db: State<'_, Db>) -> Result<AppSettings, CommandError> {
    app_settings::get(db.pool())
        .await
        .map_err(|e| AppError::Db(e).into())
}

#[tauri::command]
#[specta::specta]
pub async fn set_app_settings(
    db: State<'_, Db>,
    settings: AppSettings,
) -> Result<AppSettings, CommandError> {
    app_settings::set(db.pool(), &settings)
        .await
        .map_err(AppError::Db)?;
    Ok(settings)
}

/// Free/total bytes of the disk that holds the models directory, plus the
/// directory path itself. Drives the manager's disk row and the download
/// pane's storage meter.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct DiskUsage {
    pub models_dir: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
}

#[tauri::command]
#[specta::specta]
pub async fn disk_usage(app: AppHandle) -> Result<DiskUsage, CommandError> {
    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join(super::models::MODELS_DIR);

    let usage = tauri::async_runtime::spawn_blocking(move || {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        // The disk whose mount point is the longest prefix of the models dir
        // is the one the files actually live on.
        let disk = disks
            .list()
            .iter()
            .filter(|d| models_dir.starts_with(d.mount_point()))
            .max_by_key(|d| d.mount_point().as_os_str().len());
        let (free_bytes, total_bytes) = disk
            .map(|d| (d.available_space(), d.total_space()))
            .unwrap_or((0, 0));
        DiskUsage {
            models_dir: models_dir.to_string_lossy().into_owned(),
            free_bytes,
            total_bytes,
        }
    })
    .await
    .expect("disk usage task panicked");

    if usage.total_bytes == 0 {
        tracing::warn!(dir = %usage.models_dir, "disk_usage: no disk matched the models dir");
    }
    Ok(usage)
}

/// Re-hash every installed model file against the registry's SHA256
/// ("conferir integridade"). Persists the result as `last_integrity_check`
/// and returns it. Missing files count as corrupt.
#[tauri::command]
#[specta::specta]
pub async fn verify_installed_models(db: State<'_, Db>) -> Result<IntegrityCheck, CommandError> {
    let rows = registry::list(db.pool()).await.map_err(AppError::Db)?;

    let mut corrupt = Vec::new();
    for row in rows {
        let path = PathBuf::from(&row.path);
        let ok = tauri::async_runtime::spawn_blocking(move || sha256_of_file(&path))
            .await
            .expect("integrity hash task panicked")
            .map(|got| got.eq_ignore_ascii_case(&row.sha256))
            .unwrap_or(false);
        if !ok {
            tracing::warn!(model_id = %row.id, path = %row.path, "integrity check failed");
            corrupt.push(row.id);
        }
    }

    let check = IntegrityCheck {
        at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into()),
        corrupt,
    };

    let mut settings = app_settings::get(db.pool()).await.map_err(AppError::Db)?;
    settings.last_integrity_check = Some(check.clone());
    app_settings::set(db.pool(), &settings)
        .await
        .map_err(AppError::Db)?;

    Ok(check)
}

fn sha256_of_file(path: &Path) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(64 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// "Apagar todo o histórico": every conversation and (via cascade) every
/// message. Models and preferences stay.
#[tauri::command]
#[specta::specta]
pub async fn clear_conversations(db: State<'_, Db>) -> Result<(), CommandError> {
    let removed = conversations::delete_all(db.pool())
        .await
        .map_err(AppError::Db)?;
    tracing::info!(removed, "clear_conversations: history wiped");
    Ok(())
}

/// "Queimar tudo": cancels any download, unloads the model, deletes every
/// file in the models directory (including `.part` leftovers), then wipes
/// conversations, the installed-models registry, and all preferences.
///
/// File deletions are best-effort: a file still mapped by an in-flight
/// generation (Windows locks mapped files) is logged and skipped, and the
/// db rows are wiped regardless — startup reconciliation and the surfaced
/// error cover the leftovers. The wipe itself never stops halfway.
#[tauri::command]
#[specta::specta]
pub async fn clear_all_data(
    app: AppHandle,
    db: State<'_, Db>,
    inference_manager: State<'_, Arc<ModelManager>>,
    download_manager: State<'_, Arc<crate::models::download_manager::DownloadManager>>,
) -> Result<(), CommandError> {
    // Stop the writers first: an active download would re-register a model
    // into the burned registry when it completed.
    if let Some(active) = download_manager.active_id() {
        download_manager.cancel(&active);
    }
    inference_manager.unload().await.map_err(AppError::from)?;

    let rows = registry::list(db.pool()).await.map_err(AppError::Db)?;
    let mut files_left: Vec<String> = Vec::new();

    for row in &rows {
        if let Err(e) = tokio::fs::remove_file(&row.path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(path = %row.path, error = %e, "clear_all_data: file kept (in use?)");
                files_left.push(row.filename.clone());
            }
        }
        registry::remove(db.pool(), &row.id)
            .await
            .map_err(AppError::Db)?;
    }

    // Sweep whatever the registry didn't know about: `.part` downloads and
    // orphaned files.
    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join(super::models::MODELS_DIR);
    if let Ok(mut entries) = tokio::fs::read_dir(&models_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        tracing::warn!(file = %name, error = %e, "clear_all_data: file kept (in use?)");
                        files_left.push(name);
                    }
                }
            }
        }
    }

    let conversations_removed = conversations::delete_all(db.pool())
        .await
        .map_err(AppError::Db)?;
    app_settings::clear(db.pool()).await.map_err(AppError::Db)?;

    tracing::info!(
        models_removed = rows.len(),
        conversations_removed,
        files_left = files_left.len(),
        "clear_all_data: everything wiped"
    );

    if !files_left.is_empty() {
        return Err(CommandError {
            kind: "PartialClear".into(),
            message: format!(
                "tudo foi esquecido, mas {} arquivo(s) estavam em uso e ficaram no disco — feche e reabra o app para removê-los: {}",
                files_left.len(),
                files_left.join(", ")
            ),
        });
    }
    Ok(())
}
