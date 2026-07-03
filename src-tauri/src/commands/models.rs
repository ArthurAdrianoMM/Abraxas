//! Model-catalog, download, and registry commands.
//!
//! Fase 4.1 exposes `fetch_catalog`: hits the GitHub Pages catalog, validates
//! and caches it, falls back to the on-disk cache if the network is down.
//! Fase 4.2 adds `fetch_classified_catalog`: same fetch + hardware-compatibility
//! tier annotation on every model entry.
//! Fase 4.3 adds `start_model_download` / `cancel_model_download`: resumable
//! GGUF download with progress events. SHA256 verification lands in 4.4.
//! Fase 4.5 adds `list_installed_models`, `delete_model`, `is_model_installed`,
//! and `load_installed_model` (catalog-driven replacement for the old
//! transient `dev_load_model`).

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::db::Db;
use crate::error::{AppError, CommandError};
use crate::events::DownloadEvent;
use crate::hardware::cache;
use crate::inference::ModelManager;
use crate::models::catalog::{
    self, CatalogResponse, CATALOG_CACHE_FILENAME, CATALOG_URL, FETCH_TIMEOUT,
};
use crate::models::compatibility::{self, ClassifiedCatalogResponse};
use crate::models::download::{self, DownloadError};
use crate::models::download_manager::DownloadManager;
use crate::models::registry::{self, InstalledModel};

const MODELS_DIR: &str = "models";

#[tauri::command]
#[specta::specta]
pub async fn fetch_catalog(app: AppHandle) -> Result<CatalogResponse, CommandError> {
    let cache_path = app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join(CATALOG_CACHE_FILENAME);

    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .user_agent(concat!("abraxas/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::Catalog(catalog::CatalogError::Http(e.to_string())))?;

    let resp = catalog::fetch_with_cache(&client, CATALOG_URL, &cache_path)
        .await
        .map_err(AppError::Catalog)?;

    tracing::info!(
        source = ?resp.source,
        models = resp.catalog.models.len(),
        schema_version = resp.catalog.schema_version,
        "fetch_catalog invoked",
    );
    Ok(resp)
}

/// Fetch the model catalog and annotate every entry with a `CompatibilityTier`
/// derived from the detected hardware. Hardware is read from the fingerprint
/// cache (Fase 2.4) so this command stays fast on repeated calls.
#[tauri::command]
#[specta::specta]
pub async fn fetch_classified_catalog(
    app: AppHandle,
) -> Result<ClassifiedCatalogResponse, CommandError> {
    let app_data_dir = app.path().app_data_dir().map_err(AppError::from)?;
    let catalog_cache = app_data_dir.join(CATALOG_CACHE_FILENAME);
    let hw_cache = app_data_dir.join("hardware_cache.json");

    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .user_agent(concat!("abraxas/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::Catalog(catalog::CatalogError::Http(e.to_string())))?;

    // Run catalog fetch and hardware detection concurrently.
    let (catalog_resp, hw) = tokio::join!(
        catalog::fetch_with_cache(&client, CATALOG_URL, &catalog_cache),
        tauri::async_runtime::spawn_blocking(move || cache::load_or_detect(&hw_cache)),
    );

    let catalog_resp = catalog_resp.map_err(AppError::Catalog)?;
    let hw = hw.expect("hardware detection task panicked");

    let models = compatibility::classify_catalog(&catalog_resp.catalog, &hw);

    tracing::info!(
        source = ?catalog_resp.source,
        models = models.len(),
        backend = ?hw.choice.backend,
        from_cache = hw.from_cache,
        "fetch_classified_catalog invoked",
    );

    Ok(ClassifiedCatalogResponse {
        models,
        source: catalog_resp.source,
        fetched_at: catalog_resp.fetched_at,
        catalog_schema_version: catalog_resp.catalog.schema_version,
    })
}

/// Start a resumable download of `model_id`. Returns immediately; the
/// background task drives the download and reports progress through
/// `DownloadEvent`s. Rejects a second concurrent invocation with a
/// `Download::AlreadyInProgress`-style error.
#[tauri::command]
#[specta::specta]
pub async fn start_model_download(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    http: State<'_, reqwest::Client>,
    db: State<'_, Db>,
    model_id: String,
) -> Result<(), CommandError> {
    // Look up the catalog entry. The classified-catalog flow primes the cache
    // before the user can pick a model, so reading the cache here is enough.
    // If the cache is missing for any reason, surface a clear error rather
    // than silently re-fetching — that behavior belongs in the catalog flow.
    let cache_path = app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join(CATALOG_CACHE_FILENAME);
    let catalog = catalog::read_cache(&cache_path)
        .map_err(AppError::Catalog)?
        .ok_or_else(|| CommandError {
            kind: "Catalog".into(),
            message: "no catalog cache available; fetch the catalog first".into(),
        })?;
    let entry = catalog
        .models
        .iter()
        .find(|m| m.id == model_id)
        .cloned()
        .ok_or_else(|| CommandError {
            kind: "Download".into(),
            message: format!("unknown model id {model_id:?}"),
        })?;

    let cancel = manager.start(&model_id).ok_or_else(|| CommandError {
        kind: "Download".into(),
        message: format!(
            "another download is already in progress: {}",
            manager.active_id().unwrap_or_default()
        ),
    })?;

    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join(MODELS_DIR);

    let app_h = app.clone();
    let manager_h = Arc::clone(&manager);
    let http = (*http).clone();
    let id_for_task = model_id.clone();
    // Clone the pool so the background task owns its reference.
    let pool = db.pool().clone();

    let _ = DownloadEvent::Started {
        model_id: model_id.clone(),
        total_bytes: entry.size_bytes,
    }
    .emit(&app);

    tokio::spawn(async move {
        let id = id_for_task;
        let app = app_h;
        let progress_app = app.clone();
        let progress_id = id.clone();
        let on_progress = move |downloaded: u64, total: u64| {
            let _ = DownloadEvent::Progress {
                model_id: progress_id.clone(),
                downloaded_bytes: downloaded,
                total_bytes: total,
            }
            .emit(&progress_app);
        };

        let verify_app = app.clone();
        let verify_id = id.clone();
        let on_verify_progress = move |hashed: u64, total: u64| {
            let _ = DownloadEvent::Verifying {
                model_id: verify_id.clone(),
                hashed_bytes: hashed,
                total_bytes: total,
            }
            .emit(&verify_app);
        };

        let result = download::download_model(
            &http,
            &entry,
            &models_dir,
            cancel.clone(),
            on_progress,
            on_verify_progress,
        )
        .await;

        match result {
            Ok(outcome) => {
                let final_path = outcome.final_path.to_string_lossy().into_owned();

                // Persist the registry row before emitting Completed so the
                // frontend can call list_installed_models immediately on receipt.
                let installed_at = OffsetDateTime::now_utc()
                    .format(&Rfc3339)
                    .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into());
                let row = InstalledModel {
                    id: id.clone(),
                    filename: entry.filename.clone(),
                    path: final_path.clone(),
                    size_bytes: outcome.bytes_written as i64,
                    sha256: entry.sha256.clone(),
                    installed_at,
                };
                if let Err(e) = registry::insert(&pool, &row).await {
                    tracing::error!(error = %e, model_id = %id, "failed to insert installed_models row");
                }

                let _ = DownloadEvent::Completed {
                    model_id: id.clone(),
                    final_path,
                }
                .emit(&app);
            }
            Err(DownloadError::Cancelled) => {
                let _ = DownloadEvent::Cancelled {
                    model_id: id.clone(),
                }
                .emit(&app);
            }
            Err(DownloadError::ChecksumMismatch { .. }) => {
                tracing::warn!(model_id = %id, "model checksum mismatch — file deleted");
                let _ = DownloadEvent::Failed {
                    model_id: id.clone(),
                    kind: "ChecksumMismatch".into(),
                    message:
                        "Integrity check failed: downloaded file is corrupted. Please try again."
                            .into(),
                }
                .emit(&app);
            }
            Err(e) => {
                tracing::warn!(error = %e, model_id = %id, "model download failed");
                let _ = DownloadEvent::Failed {
                    model_id: id.clone(),
                    kind: "Download".into(),
                    message: e.to_string(),
                }
                .emit(&app);
            }
        }
        manager_h.finish(&id);
    });

    Ok(())
}

/// Signal cancellation of an in-flight download. The background task emits
/// `Cancelled` once it observes the flag and stops; the `.part` file is
/// retained on disk so a subsequent `start_model_download` resumes from
/// where it left off.
#[tauri::command]
#[specta::specta]
pub async fn cancel_model_download(
    manager: State<'_, Arc<DownloadManager>>,
    model_id: String,
) -> Result<(), CommandError> {
    manager.cancel(&model_id);
    Ok(())
}

/// Return all models currently recorded in the installed-models registry.
#[tauri::command]
#[specta::specta]
pub async fn list_installed_models(db: State<'_, Db>) -> Result<Vec<InstalledModel>, CommandError> {
    registry::list(db.pool())
        .await
        .map_err(|e| AppError::Db(e).into())
}

/// Delete an installed model: removes the file from disk, then the DB row.
///
/// Returns `ModelLoaded` error if the model is currently loaded in the
/// inference engine — deleting the file while it is loaded would leave the
/// backend pointing at a missing path; the caller must unload first.
///
/// If the file is already gone from disk the DB row is still removed — this
/// handles the case where the user deleted the file externally.
#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    db: State<'_, Db>,
    inference_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), CommandError> {
    // Resolve the registry row first so we have the path.
    let row = registry::get(db.pool(), &model_id)
        .await
        .map_err(AppError::Db)?;

    if let Some(ref row) = row {
        // Guard: refuse deletion while the model is loaded. Deleting the file
        // with the backend still holding it open means the next unload/reload
        // fails with a confusing NotFound error.
        if let Some(loaded) = inference_manager.current().await {
            if loaded.path.to_string_lossy() == row.path {
                return Err(CommandError {
                    kind: "ModelLoaded".into(),
                    message: format!(
                        "cannot delete {model_id:?}: model is currently loaded; unload it first"
                    ),
                });
            }
        }

        if let Err(e) = tokio::fs::remove_file(&row.path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(AppError::Io(e).into());
            }
        }
        tracing::info!(model_id = %model_id, path = %row.path, "model file deleted");
    }

    registry::remove(db.pool(), &model_id)
        .await
        .map_err(AppError::Db)?;

    Ok(())
}

/// Load an installed model into the inference engine. Replaces the temporary
/// `dev_load_model` from Fase 3.5 with a catalog-driven flow: the frontend
/// passes a `model_id`, this resolves the on-disk path through the registry
/// and hands it to `ModelManager::load`. The "one model loaded at a time"
/// invariant from Fase 3.3 is preserved by the manager itself.
#[tauri::command]
#[specta::specta]
pub async fn load_installed_model(
    app: AppHandle,
    db: State<'_, Db>,
    manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), CommandError> {
    let row = registry::get(db.pool(), &model_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| CommandError {
            kind: "Inference".into(),
            message: format!("model {model_id:?} is not installed"),
        })?;

    // Resolve the catalog entry so the manager binds this load to a chat
    // template and a context length. The catalog cache is primed on the
    // first classified-catalog fetch; if it's missing we still load the
    // model — the chat command will surface a clearer error than a panic.
    let cache_path = app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join(CATALOG_CACHE_FILENAME);
    let (chat_template, context_length) = match catalog::read_cache(&cache_path) {
        Ok(Some(cat)) => cat
            .models
            .into_iter()
            .find(|m| m.id == model_id)
            .map(|m| (Some(m.chat_template), Some(m.context_length)))
            .unwrap_or((None, None)),
        _ => (None, None),
    };

    manager
        .load_with(PathBuf::from(row.path), chat_template, context_length)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// Report which installed model (if any) is currently resident in the
/// inference engine. The manager tracks the loaded *path*; this resolves it
/// back to a registry `model_id` so the frontend can reflect ground truth on
/// startup instead of tracking its own load calls. Returns `None` when
/// nothing is loaded or the loaded path no longer maps to a registry row
/// (e.g. a legacy dev load).
#[tauri::command]
#[specta::specta]
pub async fn get_loaded_model(
    db: State<'_, Db>,
    manager: State<'_, Arc<ModelManager>>,
) -> Result<Option<String>, CommandError> {
    let Some(loaded) = manager.current().await else {
        return Ok(None);
    };
    let loaded_path = loaded.path.to_string_lossy();
    let rows = registry::list(db.pool()).await.map_err(AppError::Db)?;
    Ok(rows.into_iter().find(|r| r.path == loaded_path).map(|r| r.id))
}

/// Fast check: is `model_id` present in the installed-models registry?
/// The frontend uses this to decide whether to show "Download" or "Load".
#[tauri::command]
#[specta::specta]
pub async fn is_model_installed(db: State<'_, Db>, model_id: String) -> Result<bool, CommandError> {
    registry::exists(db.pool(), &model_id)
        .await
        .map_err(|e| AppError::Db(e).into())
}
