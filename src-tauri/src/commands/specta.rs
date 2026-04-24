//! Shared `tauri_specta` builder — single source of truth for the command
//! surface. Used both at runtime (`invoke_handler` + `mount_events`) and by
//! the integration test in `tests/export_bindings.rs` that regenerates
//! `src/lib/tauri/bindings.ts`.

use tauri_specta::{collect_commands, Builder};

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        super::app::app_info,
        super::hardware::detect_system,
    ])
}
