//! NVIDIA NVML probe (Windows + Linux).
//!
//! `Nvml::init()` dynamically loads `libnvidia-ml.so.1` / `nvml.dll`. A
//! missing driver is expected (most non-NVIDIA machines) and yields
//! `LibloadingError`, which we translate to `None` and let the orchestrator
//! fall through to the Vulkan probe.
//!
//! Multi-GPU: we pick the device with the highest VRAM rather than index 0,
//! because on laptops with an eGPU the internal card is often index 0 with
//! less VRAM than the external one.

use super::{ComputeCapability, GpuBackend};

pub fn probe() -> Option<GpuBackend> {
    let nvml = match nvml_wrapper::Nvml::init() {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(error = %e, "nvml init failed (no NVIDIA driver?)");
            return None;
        }
    };

    let count = match nvml.device_count() {
        Ok(0) => {
            tracing::info!("nvml reports 0 devices");
            return None;
        }
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(error = %e, "nvml device_count failed");
            return None;
        }
    };

    let mut best: Option<(u64, String, ComputeCapability, String)> = None;

    for i in 0..count {
        let device = match nvml.device_by_index(i) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(index = i, error = %e, "nvml device_by_index failed");
                continue;
            }
        };

        let mem = match device.memory_info() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(index = i, error = %e, "nvml memory_info failed");
                continue;
            }
        };

        let name = device.name().unwrap_or_else(|_| "NVIDIA GPU".to_owned());
        let cc = device
            .cuda_compute_capability()
            .map(|c| ComputeCapability {
                major: c.major.max(0) as u32,
                minor: c.minor.max(0) as u32,
            })
            .unwrap_or(ComputeCapability { major: 0, minor: 0 });
        let uuid = device.uuid().unwrap_or_default();

        let vram_mb = mem.total / 1_048_576;
        if best.as_ref().map(|(v, ..)| vram_mb > *v).unwrap_or(true) {
            best = Some((vram_mb, name, cc, uuid));
        }
    }

    let (vram_mb, name, compute_capability, uuid) = best?;
    Some(GpuBackend::Cuda {
        name,
        vram_mb,
        compute_capability,
        uuid,
    })
}
