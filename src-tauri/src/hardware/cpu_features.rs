//! x86 CPU-feature probing.
//!
//! AVX2 and AVX-512 are x86 SIMD extensions with no direct aarch64/ARM
//! equivalents, so non-x86 targets short-circuit to `false`. `raw-cpuid`
//! itself is gated in `Cargo.toml` to x86 targets, so it isn't in scope off
//! those platforms.

use super::system::CpuFeatures;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn detect() -> CpuFeatures {
    use raw_cpuid::CpuId;

    let ext = CpuId::new().get_extended_feature_info();
    CpuFeatures {
        avx2: ext.as_ref().map(|f| f.has_avx2()).unwrap_or(false),
        avx512f: ext.as_ref().map(|f| f.has_avx512f()).unwrap_or(false),
    }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn detect() -> CpuFeatures {
    CpuFeatures::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic() {
        let _ = detect();
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    #[test]
    fn non_x86_always_reports_false() {
        let f = detect();
        assert!(!f.avx2);
        assert!(!f.avx512f);
    }
}
