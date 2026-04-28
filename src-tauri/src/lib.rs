mod commands;
mod db;
mod error;
mod events;
// `pub` so internal binaries (e.g., `bin/llama_smoke`) can run the same
// detection + selection pipeline as the app. Not stable public API.
pub mod hardware;
pub mod inference;
mod logging;
mod models;

use std::sync::Arc;

use tauri::Manager;

/// Re-exported so the `src/bin/export_bindings.rs` binary can regenerate
/// `src/lib/tauri/bindings.ts`. Not part of the public API.
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
            app.manage(db);

            // Fase 3.4: detect hardware (cached after the first run via
            // fingerprint) and translate the resulting `BackendChoice` into
            // an `n_gpu_layers` value passed down to llama.cpp. CPU-only
            // hardware → 0 (no offload); any GPU choice → 999 (offload all
            // layers — llama.cpp clamps to the model's actual layer count).
            // Fine-grained CUDA-vs-Vulkan filtering on hosts where both
            // backends register is deferred; see `inference/llama_cpp.rs`.
            let cache_path = data_dir.join("hardware_cache.json");
            let detection = hardware::cache::load_or_detect(&cache_path);
            tracing::info!(
                backend = ?detection.choice.backend,
                reason = %detection.choice.reason,
                from_cache = detection.from_cache,
                "selected inference backend",
            );
            let gpu_layers = inference_gpu_layers(detection.choice.backend);

            let backend: Arc<dyn inference::InferenceBackend> =
                Arc::new(inference::LlamaCppBackend::new(gpu_layers));
            let manager = Arc::new(inference::ModelManager::new(backend));
            app.manage(manager);

            app.manage(Arc::new(commands::chat::GenerationRegistry::default()));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Maps the hardware-detection backend choice to llama.cpp's `n_gpu_layers`
/// load parameter. `999` is the upstream-recommended sentinel for "offload
/// all layers" — llama.cpp clamps to the model's actual layer count.
fn inference_gpu_layers(backend: hardware::selector::InferenceBackend) -> u32 {
    use hardware::selector::InferenceBackend::*;
    match backend {
        Cpu => 0,
        Metal | Cuda | Vulkan => 999,
    }
}
