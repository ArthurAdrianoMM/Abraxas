mod db;
mod error;
mod logging;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let guard = logging::init(app.handle())?;
            app.manage(guard);
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "abraxas starting");

            let data_dir = app.path().app_data_dir()?;
            let db_path = data_dir.join("abraxas.sqlite");
            let db = tauri::async_runtime::block_on(db::Db::init(&db_path))?;
            app.manage(db);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
