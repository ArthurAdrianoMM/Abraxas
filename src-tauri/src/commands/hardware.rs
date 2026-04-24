//! Hardware-detection commands.
//!
//! Exposes `detect_system` for Fase 2.1: OS + CPU + RAM only. The aggregate
//! `detect_hardware` command (system + GPU + chosen backend + reason) is a
//! 2.3 deliverable and will live alongside this one, not replace it.

use crate::hardware::system::{self, SystemInfo};

#[tauri::command]
#[specta::specta]
pub async fn detect_system() -> SystemInfo {
    let info = system::detect();
    tracing::info!(
        os_family = ?info.os.family,
        arch = %info.os.arch,
        logical_cores = info.cpu.logical_cores,
        physical_cores = info.cpu.physical_cores,
        total_mb = info.memory.total_bytes / 1_048_576,
        avx2 = info.cpu.features.avx2,
        avx512f = info.cpu.features.avx512f,
        "detect_system invoked",
    );
    info
}
