mod chat;
mod commands;
mod db;
mod error;
mod events;
// `pub` so the dev tools (e.g., `devtools/src/bin/llama_smoke.rs`) can run the
// same detection + selection pipeline as the app. Not stable public API.
pub mod hardware;
pub mod inference;
mod logging;
mod models;

use std::sync::Arc;

use tauri::Manager;

/// Re-exported so the `abraxas-devtools` `export_bindings` binary can
/// regenerate `src/lib/tauri/bindings.ts`. Not part of the public API.
#[doc(hidden)]
pub use commands::specta::builder as __specta_builder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = commands::specta::builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let guard = logging::init(app.handle())?;
            app.manage(guard);
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "abraxas starting");

            let data_dir = app.path().app_data_dir()?;
            let db_path = data_dir.join("abraxas.sqlite");
            let db = tauri::async_runtime::block_on(db::Db::init(&db_path))?;
            let removed = tauri::async_runtime::block_on(models::registry::reconcile(db.pool()))
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "registry reconcile failed");
                    0
                });
            if removed > 0 {
                tracing::info!(removed, "reconcile removed stale installed_models rows");
            }
            app.manage(db);

            // Where the installer put the ggml backend modules. Has to happen
            // before the first inference, because the ggml backend registry is
            // populated once per process. On macOS the backends are linked in
            // and this is a no-op (ADR 0001 §4.1).
            match app.path().resource_dir() {
                Ok(dir) => inference::set_bundled_backends_dir(dir),
                Err(e) => tracing::warn!(
                    error = %e,
                    "could not resolve the resource dir; ggml will search its own default paths",
                ),
            }

            // Detect hardware once, then let the inference backend apply the
            // load policy: CPU-only when no GPU exists, GPU-first with partial
            // offload fallback for Metal/CUDA/Vulkan.
            let cache_path = data_dir.join("hardware_cache.json");
            let detection = hardware::cache::load_or_detect(&cache_path);
            tracing::info!(
                backend = ?detection.choice.backend,
                reason = %detection.choice.reason,
                from_cache = detection.from_cache,
                "selected inference backend",
            );
            let backend: Arc<dyn inference::InferenceBackend> =
                Arc::new(inference_backend_for(detection.choice.backend));
            let manager = Arc::new(inference::ModelManager::new(backend));
            app.manage(manager);

            app.manage(Arc::new(commands::chat::GenerationRegistry::default()));

            // Fase 4.3: shared reqwest client for long-running downloads.
            // No overall `.timeout()` — multi-GB GGUF downloads can run for
            // hours on slow connections. Only `.connect_timeout()` is set so
            // dead servers fail fast. `fetch_catalog` keeps its own short-
            // timeout client; refactoring is out of scope here.
            let http = reqwest::Client::builder()
                .user_agent(concat!("abraxas/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("reqwest client build");
            app.manage(http);

            app.manage(Arc::new(models::download_manager::DownloadManager::new()));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn inference_backend_for(
    backend: hardware::selector::InferenceBackend,
) -> inference::LlamaCppBackend {
    use hardware::selector::InferenceBackend::*;
    match backend {
        Cpu => inference::LlamaCppBackend::new_cpu(),
        Metal | Cuda | Vulkan => inference::LlamaCppBackend::new_auto_gpu(),
    }
}
