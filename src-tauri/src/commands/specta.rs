//! Shared `tauri_specta` builder — single source of truth for the command
//! surface. Used both at runtime (`invoke_handler` + `mount_events`) and by
//! the integration test in `tests/export_bindings.rs` that regenerates
//! `src/lib/tauri/bindings.ts`.

use tauri_specta::{collect_commands, collect_events, Builder};

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            super::app::app_info,
            super::hardware::detect_system,
            super::hardware::detect_gpu,
            super::hardware::select_backend,
            super::hardware::detect_hardware,
            super::chat::dev_load_model,
            super::chat::start_generation,
            super::chat::cancel_generation,
            super::models::fetch_catalog,
            super::models::fetch_classified_catalog,
            super::models::start_model_download,
            super::models::cancel_model_download,
        ])
        .events(collect_events![
            crate::events::GenerationEvent,
            crate::events::DownloadEvent,
        ])
}
