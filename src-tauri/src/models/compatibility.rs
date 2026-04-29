//! Hardware-compatibility classification for catalog models (Fase 4.2).
//!
//! Pure functions — no I/O, no async. Takes a `ModelEntry` and a
//! `HardwareDetection` and returns a `CompatibilityTier` + `gpu_offload` flag.
//! Designed to be called after both catalog fetch and hardware detection are
//! already in memory.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::hardware::cache::HardwareDetection;
use crate::hardware::gpu::GpuBackend;
use crate::models::catalog::{Catalog, ModelEntry};

/// How well the detected hardware can run a given model.
///
/// Tiers are ordered from best to worst; `PartialOrd`/`Ord` let callers sort
/// by tier without matching exhaustively.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompatibilityTier {
    /// System RAM ≥ `recommended_ram_mb`. Expected smooth performance.
    Recommended,
    /// System RAM ≥ `min_ram_mb` but below recommended. Runs but may be slow.
    Viable,
    /// System RAM ≥ 75 % of `min_ram_mb`. May work with heavy swapping.
    Heavy,
    /// System RAM < 75 % of `min_ram_mb`. Likely OOM.
    NotSupported,
}

/// A catalog entry annotated with compatibility for the current machine.
#[derive(Debug, Clone, Serialize, Type)]
pub struct ClassifiedModel {
    pub model: ModelEntry,
    pub tier: CompatibilityTier,
    /// True when the detected GPU has enough VRAM (or is Apple Silicon unified
    /// memory) to offload the model layers. False means CPU-only execution.
    pub gpu_offload: bool,
}

/// Catalog + compatibility info returned to the frontend by
/// `fetch_classified_catalog`.
#[derive(Debug, Clone, Serialize, Type)]
pub struct ClassifiedCatalogResponse {
    pub models: Vec<ClassifiedModel>,
    pub source: crate::models::catalog::CatalogSource,
    pub fetched_at: String,
    pub catalog_schema_version: u8,
}

// ── classification logic ─────────────────────────────────────────────────────

/// Classify a single model against the detected hardware.
pub fn classify_model(entry: &ModelEntry, hw: &HardwareDetection) -> ClassifiedModel {
    let total_ram_mb = hw.system.memory.total_bytes / (1024 * 1024);

    let tier = if total_ram_mb >= entry.recommended_ram_mb {
        CompatibilityTier::Recommended
    } else if total_ram_mb >= entry.min_ram_mb {
        CompatibilityTier::Viable
    } else if total_ram_mb * 4 >= entry.min_ram_mb * 3 {
        // within 75 % of min_ram_mb — borderline, might work with swapping
        CompatibilityTier::Heavy
    } else {
        CompatibilityTier::NotSupported
    };

    let gpu_offload = gpu_offload_available(&hw.gpu, entry.min_vram_mb);

    ClassifiedModel {
        model: entry.clone(),
        tier,
        gpu_offload,
    }
}

/// Classify every model in a catalog, preserving catalog order.
pub fn classify_catalog(catalog: &Catalog, hw: &HardwareDetection) -> Vec<ClassifiedModel> {
    catalog.models.iter().map(|m| classify_model(m, hw)).collect()
}

fn gpu_offload_available(gpu: &GpuBackend, min_vram_mb: Option<u64>) -> bool {
    match (gpu, min_vram_mb) {
        // Apple Silicon: unified memory — always capable of GPU offload.
        (GpuBackend::Metal, _) => true,
        // Model has no VRAM requirement — CPU-only model, offload not applicable.
        (_, None) => false,
        // CUDA: offload iff detected VRAM meets the requirement.
        (GpuBackend::Cuda { vram_mb, .. }, Some(req)) => *vram_mb >= req,
        // Vulkan: offload iff VRAM is reported and meets the requirement.
        (GpuBackend::Vulkan { vram_mb: Some(v), .. }, Some(req)) => *v >= req,
        // Vulkan with unknown VRAM or no GPU at all.
        _ => false,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::cache::HardwareDetection;
    use crate::hardware::gpu::{ComputeCapability, GpuBackend, VulkanDeviceType, VulkanVendor};
    use crate::hardware::selector::{BackendChoice, InferenceBackend};
    use crate::hardware::system::{CpuFeatures, CpuInfo, MemoryInfo, OsFamily, OsInfo, SystemInfo};
    use crate::models::catalog::{ChatTemplate, ModelEntry};

    fn hw(total_ram_mb: u64, gpu: GpuBackend) -> HardwareDetection {
        HardwareDetection {
            system: SystemInfo {
                os: OsInfo {
                    family: OsFamily::Linux,
                    version: None,
                    arch: "x86_64".into(),
                },
                cpu: CpuInfo {
                    vendor: "GenuineIntel".into(),
                    brand: "Test CPU".into(),
                    physical_cores: 4,
                    logical_cores: 8,
                    features: CpuFeatures { avx2: true, avx512f: false },
                },
                memory: MemoryInfo {
                    total_bytes: total_ram_mb * 1024 * 1024,
                    available_bytes: total_ram_mb * 1024 * 1024,
                },
            },
            gpu,
            choice: BackendChoice {
                backend: InferenceBackend::Cpu,
                reason: "test".into(),
            },
            fingerprint: "test".into(),
            detected_at: "2026-01-01T00:00:00Z".into(),
            from_cache: false,
        }
    }

    fn model(min_ram_mb: u64, recommended_ram_mb: u64, min_vram_mb: Option<u64>) -> ModelEntry {
        ModelEntry {
            id: "test-model".into(),
            name: "Test Model".into(),
            publisher: "Test".into(),
            description: "".into(),
            license: "MIT".into(),
            tags: vec![],
            url: "https://example.com/model.gguf".into(),
            filename: "model.gguf".into(),
            size_bytes: 1_000_000,
            sha256: "a".repeat(64),
            params_b: 1.0,
            quantization: "Q4_K_M".into(),
            context_length: 2048,
            chat_template: ChatTemplate::ChatML,
            min_ram_mb,
            recommended_ram_mb,
            min_vram_mb,
        }
    }

    #[test]
    fn recommended() {
        let classified = classify_model(&model(2048, 4096, None), &hw(16384, GpuBackend::None));
        assert_eq!(classified.tier, CompatibilityTier::Recommended);
    }

    #[test]
    fn viable() {
        let classified = classify_model(&model(4096, 8192, None), &hw(6144, GpuBackend::None));
        assert_eq!(classified.tier, CompatibilityTier::Viable);
    }

    #[test]
    fn heavy() {
        // 3 GB RAM, min = 4 GB → 75 % threshold: 3072 >= 4096*3/4 = 3072 → Heavy
        let classified = classify_model(&model(4096, 8192, None), &hw(3072, GpuBackend::None));
        assert_eq!(classified.tier, CompatibilityTier::Heavy);
    }

    #[test]
    fn not_supported() {
        let classified = classify_model(&model(4096, 8192, None), &hw(1024, GpuBackend::None));
        assert_eq!(classified.tier, CompatibilityTier::NotSupported);
    }

    #[test]
    fn metal_gpu_offload_always_true() {
        let classified = classify_model(&model(2048, 4096, Some(4096)), &hw(16384, GpuBackend::Metal));
        assert!(classified.gpu_offload);
    }

    #[test]
    fn cuda_sufficient_vram() {
        let gpu = GpuBackend::Cuda {
            name: "RTX 4090".into(),
            vram_mb: 24576,
            compute_capability: ComputeCapability { major: 8, minor: 9 },
            uuid: "GPU-test".into(),
        };
        let classified = classify_model(&model(8192, 16384, Some(4096)), &hw(32768, gpu));
        assert!(classified.gpu_offload);
    }

    #[test]
    fn cuda_insufficient_vram_tier_by_ram() {
        let gpu = GpuBackend::Cuda {
            name: "GTX 1050 Ti".into(),
            vram_mb: 2048,
            compute_capability: ComputeCapability { major: 6, minor: 1 },
            uuid: "GPU-test".into(),
        };
        // VRAM too small → gpu_offload false, but RAM fine → Recommended
        let classified = classify_model(&model(4096, 8192, Some(6144)), &hw(16384, gpu));
        assert!(!classified.gpu_offload);
        assert_eq!(classified.tier, CompatibilityTier::Recommended);
    }

    #[test]
    fn vulkan_sufficient_vram() {
        let gpu = GpuBackend::Vulkan {
            vendor: VulkanVendor::Amd,
            vendor_id: 0x1002,
            name: "RX 7900 XTX".into(),
            vram_mb: Some(24576),
            device_type: VulkanDeviceType::Discrete,
        };
        let classified = classify_model(&model(4096, 8192, Some(8192)), &hw(16384, gpu));
        assert!(classified.gpu_offload);
    }

    #[test]
    fn vulkan_no_vram_info() {
        let gpu = GpuBackend::Vulkan {
            vendor: VulkanVendor::Intel,
            vendor_id: 0x8086,
            name: "Intel UHD 770".into(),
            vram_mb: None,
            device_type: VulkanDeviceType::Integrated,
        };
        let classified = classify_model(&model(4096, 8192, Some(4096)), &hw(16384, gpu));
        assert!(!classified.gpu_offload);
    }

    #[test]
    fn no_vram_requirement_offload_false() {
        let gpu = GpuBackend::Cuda {
            name: "RTX 4090".into(),
            vram_mb: 24576,
            compute_capability: ComputeCapability { major: 8, minor: 9 },
            uuid: "GPU-test".into(),
        };
        // min_vram_mb = None means CPU-only model; gpu_offload irrelevant → false
        let classified = classify_model(&model(4096, 8192, None), &hw(16384, gpu));
        assert!(!classified.gpu_offload);
    }
}
