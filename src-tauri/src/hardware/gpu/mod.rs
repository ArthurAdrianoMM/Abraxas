//! GPU detection for Fase 2.2.
//!
//! Infallible from the caller's perspective: each probe's errors become
//! `tracing::warn!` and fall through to the next option, ending in
//! `GpuBackend::None`. The enum *shape* follows CLAUDE.md §2.2; payloads
//! carry a little extra (GPU name, stable UUID, VRAM) so Fase 2.3's
//! selector, Fase 2.4's fingerprint cache, and Fase 6.1's onboarding UI
//! all have real identifiers to work with.
//!
//! Probing order on Windows/Linux: NVML (CUDA) → Vulkan → None. On macOS
//! we short-circuit to `Metal` via `cfg!(target_os)` without linking the
//! other probes.

use serde::{Deserialize, Serialize};
use specta::Type;

#[cfg(not(target_os = "macos"))]
mod nvml;
#[cfg(not(target_os = "macos"))]
mod vulkan;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GpuBackend {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Metal,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Cuda {
        name: String,
        vram_mb: u64,
        compute_capability: ComputeCapability,
        uuid: String,
    },
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Vulkan {
        vendor: VulkanVendor,
        vendor_id: u32,
        name: String,
        vram_mb: Option<u64>,
        device_type: VulkanDeviceType,
    },
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub struct ComputeCapability {
    pub major: u32,
    pub minor: u32,
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum VulkanVendor {
    Amd,
    Intel,
    Nvidia,
    Other,
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum VulkanDeviceType {
    Discrete,
    Integrated,
    Virtual,
    Cpu,
    Other,
}

pub fn detect() -> GpuBackend {
    #[cfg(target_os = "macos")]
    {
        GpuBackend::Metal
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(cuda) = nvml::probe() {
            return cuda;
        }
        if let Some(vk) = vulkan::probe() {
            return vk;
        }
        GpuBackend::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic() {
        let _ = detect();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_returns_metal() {
        assert!(matches!(detect(), GpuBackend::Metal));
    }
}
