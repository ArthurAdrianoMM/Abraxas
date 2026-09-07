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
//!
//! Custom models add two more ways into the registry besides the catalog:
//! `import_local_model` references a GGUF the user already has on disk, and
//! `resolve_model_source` + `start_url_download` fetch one from a link the
//! user pasted (a Hugging Face repo/file or any direct `.gguf` URL). Those
//! rows carry their chat template and context length themselves — read from
//! the GGUF header — so `load_installed_model` is registry-driven for every
//! model and only consults the catalog for rows that predate that.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_specta::Event;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::chat::templates::PromptTemplate;
use crate::db::Db;
use crate::error::{AppError, CommandError};
use crate::events::DownloadEvent;
use crate::hardware::cache;
use crate::inference::ModelManager;
use crate::models::catalog::{
    self, CatalogResponse, ChatTemplate, ModelEntry, CATALOG_CACHE_FILENAME, FETCH_TIMEOUT,
};
use crate::models::compatibility::{self, ClassifiedCatalogResponse};
use crate::models::download::{self, DownloadError, DownloadSpec};
use crate::models::download_manager::DownloadManager;
use crate::models::gguf::{self, GgufInfo};
use crate::models::hf::{self, ResolvedSource};
use crate::models::registry::{self, InstalledModel, InstalledTemplate, ModelSource};

pub(crate) const MODELS_DIR: &str = "models";

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

fn models_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join(MODELS_DIR))
}

fn catalog_cache_path(app: &AppHandle) -> Result<PathBuf, CommandError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join(CATALOG_CACHE_FILENAME))
}

#[tauri::command]
#[specta::specta]
pub async fn fetch_catalog(app: AppHandle) -> Result<CatalogResponse, CommandError> {
    let cache_path = catalog_cache_path(&app)?;

    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .user_agent(concat!("abraxas/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::Catalog(catalog::CatalogError::Http(e.to_string())))?;

    let resp = catalog::fetch_with_cache(&client, &catalog::catalog_url(), &cache_path)
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
    let url = catalog::catalog_url();
    let (catalog_resp, hw) = tokio::join!(
        catalog::fetch_with_cache(&client, &url, &catalog_cache),
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

/// Where a download's bytes will be registered once they land.
enum Install {
    Catalog(Box<ModelEntry>),
    Url { source_url: String },
}

/// One download to run: what to fetch, where to put it, how to register it.
struct DownloadJob {
    model_id: String,
    spec: DownloadSpec,
    models_dir: PathBuf,
    install: Install,
}

/// Drive `job` to completion in the background, reporting through
/// `DownloadEvent`s keyed by `job.model_id`, and register the file on
/// success. Shared by the catalog and the user-URL flows; they only differ in
/// what goes into the registry row.
fn spawn_download(
    app: AppHandle,
    http: reqwest::Client,
    pool: sqlx::SqlitePool,
    manager: Arc<DownloadManager>,
    cancel: download::CancelFlag,
    job: DownloadJob,
) {
    let DownloadJob {
        model_id,
        spec,
        models_dir,
        install,
    } = job;
    let _ = DownloadEvent::Started {
        model_id: model_id.clone(),
        total_bytes: spec.expected_size.unwrap_or(0),
    }
    .emit(&app);

    tokio::spawn(async move {
        let id = model_id;
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
            &spec,
            &models_dir,
            cancel.clone(),
            on_progress,
            on_verify_progress,
        )
        .await;

        match result {
            Ok(outcome) => {
                let row = match register_download(
                    &id,
                    &spec,
                    &outcome.final_path,
                    outcome.bytes_written,
                    install,
                )
                .await
                {
                    Ok(row) => row,
                    Err(e) => {
                        // A user URL that served something other than a GGUF:
                        // drop the file so the next attempt starts clean.
                        tracing::warn!(model_id = %id, error = %e, "downloaded file rejected");
                        tokio::fs::remove_file(&outcome.final_path).await.ok();
                        let _ = DownloadEvent::Failed {
                            model_id: id.clone(),
                            kind: "InvalidGguf".into(),
                            message: e.to_string(),
                        }
                        .emit(&app);
                        manager.finish(&id);
                        return;
                    }
                };

                // Persist the registry row before emitting Completed so the
                // frontend can call list_installed_models immediately on receipt.
                if let Err(e) = registry::insert(&pool, &row).await {
                    tracing::error!(error = %e, model_id = %id, "failed to insert installed_models row");
                }

                let _ = DownloadEvent::Completed {
                    model_id: id.clone(),
                    final_path: row.path,
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
        manager.finish(&id);
    });
}

/// Build the registry row for a finished download. Catalog entries bring
/// their own metadata; a user URL is described by the GGUF header, which is
/// also what rejects a download that turned out not to be a GGUF at all.
async fn register_download(
    id: &str,
    spec: &DownloadSpec,
    final_path: &Path,
    bytes_written: u64,
    install: Install,
) -> Result<InstalledModel, AppError> {
    let path = final_path.to_string_lossy().into_owned();
    Ok(match install {
        Install::Catalog(entry) => InstalledModel {
            id: id.to_owned(),
            filename: entry.filename.clone(),
            path,
            size_bytes: bytes_written as i64,
            sha256: Some(entry.sha256.clone()),
            installed_at: now_rfc3339(),
            source: ModelSource::Catalog,
            source_url: Some(entry.url.clone()),
            display_name: Some(entry.name.clone()),
            chat_template: Some(InstalledTemplate::Family(entry.chat_template)),
            context_length: Some(entry.context_length),
        },
        Install::Url { source_url } => {
            let inspect_path = final_path.to_path_buf();
            let info = tauri::async_runtime::spawn_blocking(move || gguf::inspect(&inspect_path))
                .await
                .expect("gguf inspect task panicked")?;
            custom_row(
                id,
                &spec.filename,
                path,
                ModelSource::Url,
                Some(source_url),
                &info,
                None,
            )
        }
    })
}

/// Registry row for a model the catalog knows nothing about. The display
/// name falls back to the file name; the template is the embedded one when
/// llama.cpp can render it, else the caller's override, else undecided.
fn custom_row(
    id: &str,
    filename: &str,
    path: String,
    source: ModelSource,
    source_url: Option<String>,
    info: &GgufInfo,
    template_override: Option<ChatTemplate>,
) -> InstalledModel {
    let chat_template = match template_override {
        Some(family) => Some(InstalledTemplate::Family(family)),
        None if info.embedded_template_supported => Some(InstalledTemplate::Embedded),
        None => None,
    };
    InstalledModel {
        id: id.to_owned(),
        filename: filename.to_owned(),
        path,
        size_bytes: info.size_bytes as i64,
        sha256: None,
        installed_at: now_rfc3339(),
        source,
        source_url,
        display_name: Some(
            info.name
                .clone()
                .unwrap_or_else(|| filename.trim_end_matches(".gguf").to_owned()),
        ),
        chat_template,
        context_length: info.context_length,
    }
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
    let catalog = catalog::read_cache(&catalog_cache_path(&app)?)
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

    spawn_download(
        app.clone(),
        (*http).clone(),
        db.pool().clone(),
        Arc::clone(&manager),
        cancel,
        DownloadJob {
            model_id,
            spec: DownloadSpec::from(&entry),
            models_dir: models_dir(&app)?,
            install: Install::Catalog(Box::new(entry)),
        },
    );
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
///
/// A `Local` model is the user's own file, referenced in place: only the row
/// is removed, the file is left where it was.
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

        if row.source == ModelSource::Local {
            tracing::info!(model_id = %model_id, path = %row.path, "local model forgotten; file kept");
        } else {
            if let Err(e) = tokio::fs::remove_file(&row.path).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(AppError::Io(e).into());
                }
            }
            tracing::info!(model_id = %model_id, path = %row.path, "model file deleted");
        }
    }

    registry::remove(db.pool(), &model_id)
        .await
        .map_err(AppError::Db)?;

    Ok(())
}

/// Load an installed model into the inference engine. The frontend passes a
/// `model_id`; this resolves the on-disk path, chat template and context
/// length through the registry and hands them to `ModelManager::load`. The
/// "one model loaded at a time" invariant from Fase 3.3 is preserved by the
/// manager itself.
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

    let (chat_template, context_length) = template_for(&app, &row).await?;

    manager
        .load_with(PathBuf::from(row.path), chat_template, context_length)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// Resolve how to render prompts for `row` and how much context it has.
///
/// Rows written since migration 0006 carry both. Catalog rows from before
/// it have neither and are looked up in the catalog cache, exactly as the
/// old catalog-driven load did; if the cache is gone too the model still
/// loads and the chat command surfaces a clear error.
async fn template_for(
    app: &AppHandle,
    row: &InstalledModel,
) -> Result<(Option<PromptTemplate>, Option<u32>), CommandError> {
    let template = match row.chat_template {
        Some(InstalledTemplate::Family(family)) => Some(PromptTemplate::Family(family)),
        Some(InstalledTemplate::Embedded) => {
            let path = PathBuf::from(&row.path);
            tauri::async_runtime::spawn_blocking(move || gguf::embedded_chat_template(&path))
                .await
                .expect("gguf template task panicked")
                .map_err(AppError::from)?
                .map(PromptTemplate::Embedded)
        }
        None => None,
    };

    if template.is_some() || row.source != ModelSource::Catalog {
        return Ok((template, row.context_length));
    }

    // Legacy catalog row: fall back to the catalog cache.
    let from_catalog = match catalog::read_cache(&catalog_cache_path(app)?) {
        Ok(Some(cat)) => cat.models.into_iter().find(|m| m.id == row.id).map(|m| {
            (
                Some(PromptTemplate::Family(m.chat_template)),
                Some(m.context_length),
            )
        }),
        _ => None,
    };
    Ok(from_catalog.unwrap_or((None, row.context_length)))
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
    Ok(rows
        .into_iter()
        .find(|r| r.path == loaded_path)
        .map(|r| r.id))
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

// ── custom models ────────────────────────────────────────────────────────────

/// Open the native file picker filtered to `.gguf`. Returns the chosen path,
/// or `None` if the user dismissed the dialog.
#[tauri::command]
#[specta::specta]
pub async fn pick_gguf_file(app: AppHandle) -> Result<Option<String>, CommandError> {
    // `blocking_pick_file` must not run on the main thread; a Tauri command
    // runs on the async runtime, and the plugin hops to the main thread
    // itself where the OS requires it.
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("GGUF model", &["gguf"])
            .blocking_pick_file()
    })
    .await
    .expect("file picker task panicked");

    Ok(picked.map(|p| p.to_string()))
}

/// Register a GGUF that already exists on disk, in place — no copy. The file
/// is validated by reading its header, which also yields the name, context
/// length and whether its embedded chat template is usable. Importing the
/// same file twice updates the existing row instead of creating a second.
///
/// `template` overrides the chat template (for GGUFs whose embedded template
/// llama.cpp can't render, or when the user knows better).
#[tauri::command]
#[specta::specta]
pub async fn import_local_model(
    db: State<'_, Db>,
    path: String,
    template: Option<ChatTemplate>,
) -> Result<InstalledModel, CommandError> {
    let path = PathBuf::from(path);
    let canonical = tokio::fs::canonicalize(&path)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                AppError::Gguf(gguf::GgufError::NotFound(path.display().to_string()))
            }
            _ => AppError::Io(e),
        })?;
    let inspect_path = canonical.clone();
    let info = tauri::async_runtime::spawn_blocking(move || gguf::inspect(&inspect_path))
        .await
        .expect("gguf inspect task panicked")
        .map_err(AppError::from)?;

    let path_str = canonical.to_string_lossy().into_owned();
    // Same file, same row — whichever flow brought it in first.
    let id = match registry::get_by_path(db.pool(), &path_str)
        .await
        .map_err(AppError::Db)?
    {
        Some(existing) => existing.id,
        None => format!("local:{}", short_hash(&path_str)),
    };
    let filename = canonical
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model.gguf".into());

    let row = custom_row(
        &id,
        &filename,
        path_str,
        ModelSource::Local,
        None,
        &info,
        template,
    );
    registry::insert(db.pool(), &row)
        .await
        .map_err(AppError::Db)?;
    tracing::info!(
        model_id = %row.id,
        path = %row.path,
        architecture = %info.architecture,
        template = ?row.chat_template,
        "local model imported"
    );
    Ok(row)
}

/// First 16 hex chars of SHA-256 — a stable id for a path or URL without
/// leaking the whole string into the id.
fn short_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Turn a pasted link into the GGUF files it offers, each annotated with an
/// estimated compatibility tier so the UI can point at the one that fits.
#[tauri::command]
#[specta::specta]
pub async fn resolve_model_source(
    app: AppHandle,
    http: State<'_, reqwest::Client>,
    input: String,
) -> Result<ResolvedSource, CommandError> {
    let source = hf::parse_source(&input).map_err(AppError::from)?;
    let hw_cache = app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join("hardware_cache.json");

    let (resolved, hw) = tokio::join!(
        hf::resolve(&http, &source),
        tauri::async_runtime::spawn_blocking(move || cache::load_or_detect(&hw_cache)),
    );
    let mut resolved = resolved.map_err(AppError::from)?;
    let hw = hw.expect("hardware detection task panicked");

    for file in &mut resolved.files {
        file.tier = file
            .size_bytes
            .map(|bytes| compatibility::classify_size(bytes, &hw));
    }

    tracing::info!(
        repo = ?resolved.repo,
        files = resolved.files.len(),
        "resolve_model_source invoked"
    );
    Ok(resolved)
}

/// One file picked from a `ResolvedSource`, as the frontend sends it back.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct UrlDownloadRequest {
    /// Direct download URL (`RemoteGguf::url`).
    pub url: String,
    /// File name to save under (`RemoteGguf::filename`).
    pub filename: String,
    /// The link the user pasted, remembered on the row
    /// (`ResolvedSource::source_url`).
    pub source_url: String,
    /// Size announced by the listing, for progress before the first byte.
    pub expected_size: Option<u64>,
}

/// Download one of the files from `resolve_model_source`. Returns the
/// `model_id` the `DownloadEvent`s will be keyed by. No checksum is known for
/// a user URL, so the file is not verified; its GGUF header is read once the
/// bytes land, and a file that isn't a GGUF is discarded with an
/// `InvalidGguf` failure event.
#[tauri::command]
#[specta::specta]
pub async fn start_url_download(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    http: State<'_, reqwest::Client>,
    db: State<'_, Db>,
    request: UrlDownloadRequest,
) -> Result<String, CommandError> {
    let UrlDownloadRequest {
        url,
        filename,
        source_url,
        expected_size,
    } = request;
    if !url.starts_with("https://") {
        return Err(CommandError {
            kind: "Source".into(),
            message: "download links must use https".into(),
        });
    }
    // The filename is written into the models dir: keep it a plain file name.
    if filename.is_empty()
        || filename.contains(['/', '\\'])
        || filename == "."
        || filename == ".."
        || !filename.to_ascii_lowercase().ends_with(".gguf")
    {
        return Err(CommandError {
            kind: "Source".into(),
            message: format!("invalid file name {filename:?}"),
        });
    }
    if hf::is_split_shard(&filename) {
        return Err(CommandError {
            kind: "Source".into(),
            message: "split GGUFs (…-00001-of-0000N.gguf) are not supported; pick a single-file quantization".into(),
        });
    }

    let models_dir = models_dir(&app)?;
    let final_path = models_dir.join(&filename).to_string_lossy().into_owned();
    let model_id = format!("url:{}", short_hash(&url));

    // Another row already owns this file name (e.g. the same quantization
    // came from the catalog). Two rows on one file would break deletion.
    if let Some(other) = registry::get_by_path(db.pool(), &final_path)
        .await
        .map_err(AppError::Db)?
    {
        if other.id != model_id {
            return Err(CommandError {
                kind: "AlreadyInstalled".into(),
                message: format!(
                    "{filename} is already installed as {}",
                    other.display_name.unwrap_or(other.id)
                ),
            });
        }
    }

    let cancel = manager.start(&model_id).ok_or_else(|| CommandError {
        kind: "Download".into(),
        message: format!(
            "another download is already in progress: {}",
            manager.active_id().unwrap_or_default()
        ),
    })?;

    spawn_download(
        app.clone(),
        (*http).clone(),
        db.pool().clone(),
        Arc::clone(&manager),
        cancel,
        DownloadJob {
            model_id: model_id.clone(),
            spec: DownloadSpec {
                url,
                filename,
                expected_size,
                expected_sha256: None,
            },
            models_dir,
            install: Install::Url { source_url },
        },
    );
    Ok(model_id)
}

/// Choose how prompts are rendered for an installed model: a family, or
/// `None` to go back to the GGUF's embedded template. Takes effect at once
/// if that model is the one loaded.
#[tauri::command]
#[specta::specta]
pub async fn set_model_chat_template(
    db: State<'_, Db>,
    manager: State<'_, Arc<ModelManager>>,
    model_id: String,
    template: Option<ChatTemplate>,
) -> Result<InstalledModel, CommandError> {
    let row = registry::get(db.pool(), &model_id)
        .await
        .map_err(AppError::Db)?
        .ok_or_else(|| CommandError {
            kind: "Inference".into(),
            message: format!("model {model_id:?} is not installed"),
        })?;

    let path = PathBuf::from(&row.path);
    let installed = match template {
        Some(family) => Some(InstalledTemplate::Family(family)),
        None => {
            let inspect_path = path.clone();
            let info = tauri::async_runtime::spawn_blocking(move || gguf::inspect(&inspect_path))
                .await
                .expect("gguf inspect task panicked")
                .map_err(AppError::from)?;
            info.embedded_template_supported
                .then_some(InstalledTemplate::Embedded)
        }
    };

    registry::set_chat_template(db.pool(), &model_id, installed)
        .await
        .map_err(AppError::Db)?;

    let live = match installed {
        Some(InstalledTemplate::Family(family)) => Some(PromptTemplate::Family(family)),
        Some(InstalledTemplate::Embedded) => {
            let p = path.clone();
            tauri::async_runtime::spawn_blocking(move || gguf::embedded_chat_template(&p))
                .await
                .expect("gguf template task panicked")
                .map_err(AppError::from)?
                .map(PromptTemplate::Embedded)
        }
        None => None,
    };
    manager.set_chat_template(&path, live).await;

    Ok(InstalledModel {
        chat_template: installed,
        ..row
    })
}
