//! Model-catalog and download commands.
//!
//! Fase 4.1 exposes `fetch_catalog`: hits the GitHub Pages catalog, validates
//! and caches it, falls back to the on-disk cache if the network is down.
//! Fase 4.2 adds `fetch_classified_catalog`: same fetch + hardware-compatibility
//! tier annotation on every model entry.
//! Download/install commands land with Fase 4.3+.

use tauri::{AppHandle, Manager};

use crate::error::{AppError, CommandError};
use crate::hardware::cache;
use crate::models::catalog::{
    self, CatalogResponse, CATALOG_CACHE_FILENAME, CATALOG_URL, FETCH_TIMEOUT,
};
use crate::models::compatibility::{self, ClassifiedCatalogResponse};

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
