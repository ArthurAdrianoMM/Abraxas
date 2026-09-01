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
//!
//! A detected NVIDIA GPU is only reported as `Cuda` when the release binary
//! actually carries kernels for it — see `ComputeCapability::has_cuda_kernel`.
//! Otherwise the probe falls through to Vulkan, which every NVIDIA driver
//! ships an ICD for.

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

impl ComputeCapability {
    /// Whether the release binary carries CUDA kernels this GPU can run.
    ///
    /// Mirrors `CMAKE_CUDA_ARCHITECTURES` in `.github/workflows/release.yml`,
    /// which is `61-real;75-real;86-real;89-real;120-real;90-virtual`. The two
    /// lists are a pair: changing the workflow without changing this function
    /// makes the app promise CUDA to a GPU that has no kernel to run, and ggml
    /// then dies with "no kernel image is available for execution on the
    /// device" — after the model is already loading, which reads as a crash.
    ///
    /// CUDA cubins are binary-compatible upward within a major version only
    /// (an `sm_86` cubin runs on `sm_87` and `sm_89`, never on `sm_80`), so
    /// each arch in the list covers itself and higher minors of that major.
    /// The single `90-virtual` entry embeds PTX, which the driver JIT-compiles
    /// for any `>= 9.0` device — that is what covers Hopper and whatever ships
    /// after Blackwell.
    ///
    /// Deliberately uncovered, and therefore routed to Vulkan: Maxwell (5.x),
    /// P100 (6.0), Volta (7.0/7.2) and A100 (8.0). None of them is this
    /// project's target hardware (CLAUDE.md §1.4) and each would cost another
    /// ~100-150 MB of cubin in the installer.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub fn has_cuda_kernel(&self) -> bool {
        match (self.major, self.minor) {
            // sm_61 cubin: Pascal GTX 10xx (also sm_62).
            (6, minor) => minor >= 1,
            // sm_75 cubin: Turing RTX 20xx / GTX 16xx.
            (7, minor) => minor >= 5,
            // sm_86 cubin: Ampere RTX 30xx, covering sm_87 and Ada sm_89.
            (8, minor) => minor >= 6,
            // sm_120 cubin (Blackwell RTX 50xx) plus compute_90 PTX for the
            // rest of the >= 9.0 range, JIT-compiled by the driver.
            (major, _) => major >= 9,
        }
    }
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
        // Discard a CUDA GPU this build has no kernel for and keep probing:
        // NVML saw the card and the driver is fine, so Vulkan — not CPU — is
        // the right answer, and that is the next probe in line.
        let cuda = nvml::probe().filter(|gpu| match gpu {
            GpuBackend::Cuda {
                name,
                compute_capability,
                ..
            } => {
                let usable = compute_capability.has_cuda_kernel();
                if !usable {
                    tracing::warn!(
                        gpu = %name,
                        compute_major = compute_capability.major,
                        compute_minor = compute_capability.minor,
                        "NVIDIA GPU has no CUDA kernel in this build; falling back to Vulkan",
                    );
                }
                usable
            }
            _ => true,
        });
        if let Some(cuda) = cuda {
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

    fn cc(major: u32, minor: u32) -> ComputeCapability {
        ComputeCapability { major, minor }
    }

    #[test]
    fn shipped_cuda_architectures_have_kernels() {
        // One real GPU per arch in `CMAKE_CUDA_ARCHITECTURES`, plus the minor
        // bumps each cubin covers for free.
        for (major, minor, gpu) in [
            (6, 1, "GTX 1080"),
            (6, 2, "Jetson TX2"),
            (7, 5, "RTX 2070"),
            (8, 6, "RTX 3080"),
            (8, 7, "Jetson Orin"),
            (8, 9, "RTX 4090"),
            (9, 0, "H100 (via compute_90 PTX)"),
            (12, 0, "RTX 5090"),
            (12, 1, "RTX 5070"),
        ] {
            assert!(
                cc(major, minor).has_cuda_kernel(),
                "{gpu} (sm_{major}{minor}) must have a kernel in this build",
            );
        }
    }

    #[test]
    fn unshipped_cuda_architectures_have_no_kernel() {
        // Deliberately left out of the arch list; these must route to Vulkan
        // rather than be promised CUDA.
        for (major, minor, gpu) in [
            (5, 0, "GTX 750 Ti"),
            (5, 2, "GTX 980"),
            (6, 0, "Tesla P100"),
            (7, 0, "Titan V / V100"),
            (7, 2, "Jetson Xavier"),
            (8, 0, "A100"),
        ] {
            assert!(
                !cc(major, minor).has_cuda_kernel(),
                "{gpu} (sm_{major}{minor}) has no cubin and no compatible PTX",
            );
        }
    }
}
