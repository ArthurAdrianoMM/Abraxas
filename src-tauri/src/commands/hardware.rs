//! Hardware-detection commands.
//!
//! Fase 2.1 exposes `detect_system` (OS + CPU + RAM). Fase 2.2 adds
//! `detect_gpu`. Fase 2.3 adds the pure `select_backend`. Fase 2.4 adds
//! the aggregate `detect_hardware`, which is the canonical entry point —
//! it composes the three with a fingerprint-validated JSON cache so cold
//! NVML/Vulkan init isn't re-run on every launch. The granular commands
//! stay around for dev tools.

use tauri::{AppHandle, Manager};

use crate::error::{AppError, CommandError};
use crate::hardware::cache::{self, HardwareDetection};
use crate::hardware::gpu::{self, GpuBackend};
use crate::hardware::selector::{self, BackendChoice};
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

/// Pure backend-selection command. Takes already-detected `SystemInfo` and
/// `GpuBackend` from the frontend and returns the chosen inference backend
/// plus a short justification. Kept exposed for dev tools; the standard
/// flow uses `detect_hardware` instead.
#[tauri::command]
#[specta::specta]
pub async fn select_backend(system: SystemInfo, gpu: GpuBackend) -> BackendChoice {
    let choice = selector::select_backend(&system, &gpu);
    tracing::info!(backend = ?choice.backend, reason = %choice.reason, "select_backend invoked");
    choice
}

/// Aggregate detection with a fingerprint-validated JSON cache. Returns
/// the full (system, gpu, choice) triple. When `force` is false, a cached
/// result is served if the host's CPU/RAM/OS fingerprint still matches;
/// when true, fresh detection runs unconditionally and the cache file is
/// overwritten. The blocking work (file I/O + GPU probe) is dispatched
/// to `spawn_blocking` so the Tauri IPC thread stays free.
#[tauri::command]
#[specta::specta]
pub async fn detect_hardware(
    app: AppHandle,
    force: bool,
) -> Result<HardwareDetection, CommandError> {
    let cache_path = app
        .path()
        .app_data_dir()
        .map_err(AppError::from)?
        .join("hardware_cache.json");

    let detection = tauri::async_runtime::spawn_blocking(move || {
        if force {
            cache::force_redetect(&cache_path)
        } else {
            cache::load_or_detect(&cache_path)
        }
    })
    .await
    .expect("hardware detection task panicked");

    tracing::info!(
        from_cache = detection.from_cache,
        backend = ?detection.choice.backend,
        fingerprint = %detection.fingerprint,
        "detect_hardware invoked",
    );
    Ok(detection)
}
