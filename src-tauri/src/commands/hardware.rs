//! Hardware-detection commands.
//!
//! Fase 2.1 exposes `detect_system` (OS + CPU + RAM). Fase 2.2 adds
//! `detect_gpu`. The aggregate `detect_hardware` command (system + GPU +
//! chosen backend + reason) is a 2.3 deliverable and will live alongside
//! these two, not replace them.

use crate::hardware::gpu::{self, GpuBackend};
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

/// GPU probe. Runs in `spawn_blocking` because NVML init and Vulkan instance
/// creation can each take a few hundred ms on cold driver load, and we don't
/// want to stall the Tauri IPC thread. `tauri::async_runtime` wraps tokio
/// so we don't need a direct tokio dep.
#[tauri::command]
#[specta::specta]
pub async fn detect_gpu() -> GpuBackend {
    let gpu = tauri::async_runtime::spawn_blocking(gpu::detect)
        .await
        .unwrap_or(GpuBackend::None);
    tracing::info!(?gpu, "detect_gpu invoked");
    gpu
}
