//! Shared `tauri_specta` builder — single source of truth for the command
//! surface. Used both at runtime (`invoke_handler` + `mount_events`) and by
//! the `abraxas-devtools` `export_bindings` binary that regenerates
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
            super::chat::start_generation,
            super::chat::cancel_generation,
            super::conversations::create_conversation,
            super::conversations::list_conversations,
            super::conversations::delete_conversation,
            super::conversations::update_conversation_generation_params,
            super::conversations::append_message,
            super::conversations::list_messages,
            super::models::fetch_catalog,
            super::models::fetch_classified_catalog,
            super::models::start_model_download,
            super::models::cancel_model_download,
            super::models::list_installed_models,
            super::models::delete_model,
            super::models::is_model_installed,
            super::models::load_installed_model,
            super::models::get_loaded_model,
            super::settings::get_app_settings,
            super::settings::set_app_settings,
            super::settings::disk_usage,
            super::settings::verify_installed_models,
            super::settings::clear_conversations,
            super::settings::clear_all_data,
        ])
        .events(collect_events![
            crate::events::GenerationEvent,
            crate::events::DownloadEvent,
        ])
}
