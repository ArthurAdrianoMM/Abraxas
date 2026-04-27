mod commands;
mod db;
mod error;
mod hardware;
pub mod inference;
mod logging;

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

            // Fase 3.4 will branch this construction on cfg flags to pick a
            // Metal/CUDA/Vulkan-built backend; Fase 3.5 consumes the manager
            // through `State<Arc<ModelManager>>` in the chat commands.
            let backend: Arc<dyn inference::InferenceBackend> =
                Arc::new(inference::LlamaCppBackend::new());
            let manager = Arc::new(inference::ModelManager::new(backend));
            app.manage(manager);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
