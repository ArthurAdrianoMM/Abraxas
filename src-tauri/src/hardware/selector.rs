//! Backend-selection logic (Fase 2.3).
//!
//! Pure function: translates a `(SystemInfo, GpuBackend)` pair into a
//! single inference-backend choice plus a short, user-facing reason. No
//! I/O, no side effects — every branch is covered by unit tests that run
//! on every platform.
//!
//! OS-specific rules (macOS → Metal, Windows/Linux → CUDA or Vulkan) are
//! already encoded upstream in `hardware::gpu::detect`: on macOS the GPU
//! probe is cfg-gated to always return `Metal`, and `Cuda` / `Vulkan`
//! variants only appear on Windows/Linux. The selector is the pure
//! consumer of that contract, so it just matches on `GpuBackend` without
//! re-validating OS combinations.
//!
//! `system` is part of the public signature even though it's currently
//! unused in the body — Fase 2.4+ will extend `reason` with RAM context
//! and may add minimum-RAM gating. Stable signature now = no churn later.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::gpu::GpuBackend;
use super::system::SystemInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum InferenceBackend {
    Metal,
    Cuda,
    Vulkan,
    Cpu,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BackendChoice {
    pub backend: InferenceBackend,
    pub reason: String,
}

pub fn select_backend(_system: &SystemInfo, gpu: &GpuBackend) -> BackendChoice {
    match gpu {
        GpuBackend::Metal => BackendChoice {
            backend: InferenceBackend::Metal,
            reason: "Metal available on macOS".to_owned(),
        },
        GpuBackend::Cuda {
            name,
            vram_mb,
            compute_capability,
            ..
        } => BackendChoice {
            backend: InferenceBackend::Cuda,
            reason: format!(
                "CUDA GPU: {name} ({vram_mb} MB, compute {}.{})",
                compute_capability.major, compute_capability.minor,
            ),
        },
        GpuBackend::Vulkan { vendor, name, .. } => BackendChoice {
            backend: InferenceBackend::Vulkan,
            reason: format!("Vulkan GPU: {name} ({vendor:?})"),
        },
        GpuBackend::None => BackendChoice {
            backend: InferenceBackend::Cpu,
            reason: "No compatible GPU detected, falling back to CPU".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::gpu::{ComputeCapability, VulkanDeviceType, VulkanVendor};
    use crate::hardware::system::{CpuFeatures, CpuInfo, MemoryInfo, OsFamily, OsInfo, SystemInfo};

    fn fake_system() -> SystemInfo {
        SystemInfo {
            os: OsInfo {
                family: OsFamily::Other,
                version: None,
                arch: "x86_64".to_owned(),
            },
            cpu: CpuInfo {
                vendor: String::new(),
                brand: String::new(),
                physical_cores: 1,
                logical_cores: 1,
                features: CpuFeatures::default(),
            },
            memory: MemoryInfo {
                total_bytes: 8 * 1_073_741_824,
                available_bytes: 4 * 1_073_741_824,
            },
        }
    }

    fn fake_cuda() -> GpuBackend {
        GpuBackend::Cuda {
            name: "RTX 4090".to_owned(),
            vram_mb: 24_576,
            compute_capability: ComputeCapability { major: 8, minor: 9 },
            uuid: "GPU-00000000-0000-0000-0000-000000000000".to_owned(),
        }
    }

    fn fake_vulkan(vendor: VulkanVendor) -> GpuBackend {
        GpuBackend::Vulkan {
            vendor,
            vendor_id: 0x1002,
            name: "Test Vulkan Device".to_owned(),
            vram_mb: Some(8_192),
            device_type: VulkanDeviceType::Discrete,
        }
    }

    #[test]
    fn metal_backend_selects_metal() {
        let choice = select_backend(&fake_system(), &GpuBackend::Metal);
        assert_eq!(choice.backend, InferenceBackend::Metal);
    }

    #[test]
    fn cuda_backend_selects_cuda() {
        let choice = select_backend(&fake_system(), &fake_cuda());
        assert_eq!(choice.backend, InferenceBackend::Cuda);
        assert!(
            choice.reason.contains("RTX 4090"),
            "reason should mention GPU name, got: {}",
            choice.reason,
        );
        assert!(
            choice.reason.contains("24576"),
            "reason should mention VRAM in MB, got: {}",
            choice.reason,
        );
        assert!(
            choice.reason.contains("8.9"),
            "reason should mention compute capability, got: {}",
            choice.reason,
        );
    }

    #[test]
    fn vulkan_amd_selects_vulkan() {
        let choice = select_backend(&fake_system(), &fake_vulkan(VulkanVendor::Amd));
        assert_eq!(choice.backend, InferenceBackend::Vulkan);
        assert!(choice.reason.contains("Amd"), "reason: {}", choice.reason);
    }

    #[test]
    fn vulkan_intel_selects_vulkan() {
        let choice = select_backend(&fake_system(), &fake_vulkan(VulkanVendor::Intel));
        assert_eq!(choice.backend, InferenceBackend::Vulkan);
    }

    #[test]
    fn vulkan_nvidia_selects_vulkan() {
        // Edge case: NVML failed (no CUDA driver) but Vulkan saw the Nvidia card.
        let choice = select_backend(&fake_system(), &fake_vulkan(VulkanVendor::Nvidia));
        assert_eq!(choice.backend, InferenceBackend::Vulkan);
    }

    #[test]
    fn vulkan_other_vendor_selects_vulkan() {
        let choice = select_backend(&fake_system(), &fake_vulkan(VulkanVendor::Other));
        assert_eq!(choice.backend, InferenceBackend::Vulkan);
    }

    #[test]
    fn none_selects_cpu() {
        let choice = select_backend(&fake_system(), &GpuBackend::None);
        assert_eq!(choice.backend, InferenceBackend::Cpu);
    }

    #[test]
    fn reason_string_is_non_empty_for_all_variants() {
        let sys = fake_system();
        for gpu in [
            GpuBackend::Metal,
            fake_cuda(),
            fake_vulkan(VulkanVendor::Amd),
            fake_vulkan(VulkanVendor::Intel),
            fake_vulkan(VulkanVendor::Nvidia),
            fake_vulkan(VulkanVendor::Other),
            GpuBackend::None,
        ] {
            let choice = select_backend(&sys, &gpu);
            assert!(
                !choice.reason.trim().is_empty(),
                "reason must be non-empty for {:?}",
                choice.backend,
            );
        }
    }
}
