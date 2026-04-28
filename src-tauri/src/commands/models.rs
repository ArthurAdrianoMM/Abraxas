//! Model-catalog and download commands.
//!
//! Fase 4.1 exposes `fetch_catalog`: hits the GitHub Pages catalog, validates
//! and caches it, falls back to the on-disk cache if the network is down.
//! Download/install commands land with Fase 4.3+.

use tauri::{AppHandle, Manager};

use crate::error::{AppError, CommandError};
use crate::models::catalog::{
    self, CatalogResponse, CATALOG_CACHE_FILENAME, CATALOG_URL, FETCH_TIMEOUT,
};

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
