//! OS + CPU + RAM detection via `sysinfo` + `raw-cpuid`.
//!
//! `detect()` is infallible: probing host state either yields a value or
//! sensible defaults (e.g. empty brand string, `OsFamily::Other`). Later
//! phases (2.2 GPU, 2.3 selector) read `SystemInfo` top-down without reaching
//! into its internals, so the struct shape is the stable contract.

use serde::{Deserialize, Serialize};
use specta::Type;
use sysinfo::{CpuRefreshKind, System};

use super::cpu_features;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SystemInfo {
    pub os: OsInfo,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OsInfo {
    pub family: OsFamily,
    pub version: Option<String>,
    pub arch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum OsFamily {
    Windows,
    MacOs,
    Linux,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CpuInfo {
    pub vendor: String,
    pub brand: String,
    pub physical_cores: u32,
    pub logical_cores: u32,
    pub features: CpuFeatures,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CpuFeatures {
    pub avx2: bool,
    pub avx512f: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn detect() -> SystemInfo {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_list(CpuRefreshKind::everything());

    let logical_cores = sys.cpus().len() as u32;
    let physical_cores = sys
        .physical_core_count()
        .map(|n| n as u32)
        .unwrap_or(logical_cores);

    let (vendor, brand) = sys
        .cpus()
        .first()
        .map(|cpu| (cpu.vendor_id().to_owned(), cpu.brand().trim().to_owned()))
        .unwrap_or_default();

    SystemInfo {
        os: OsInfo {
            family: os_family(),
            version: System::long_os_version(),
            arch: std::env::consts::ARCH.to_owned(),
        },
        cpu: CpuInfo {
            vendor,
            brand,
            physical_cores,
            logical_cores,
            features: cpu_features::detect(),
        },
        memory: MemoryInfo {
            total_bytes: sys.total_memory(),
            available_bytes: sys.available_memory(),
        },
    }
}

fn os_family() -> OsFamily {
    match std::env::consts::OS {
        "windows" => OsFamily::Windows,
        "macos" => OsFamily::MacOs,
        "linux" => OsFamily::Linux,
        _ => OsFamily::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_at_least_one_core() {
        let info = detect();
        assert!(info.cpu.logical_cores >= 1, "logical_cores must be >= 1");
        assert!(info.cpu.physical_cores >= 1, "physical_cores must be >= 1");
        assert!(
            info.cpu.physical_cores <= info.cpu.logical_cores,
            "physical ({}) cannot exceed logical ({})",
            info.cpu.physical_cores,
            info.cpu.logical_cores,
        );
    }

    #[test]
    fn detect_returns_nonzero_total_memory() {
        let info = detect();
        assert!(info.memory.total_bytes > 0, "total_bytes must be > 0");
        assert!(
            info.memory.available_bytes <= info.memory.total_bytes,
            "available ({}) cannot exceed total ({})",
            info.memory.available_bytes,
            info.memory.total_bytes,
        );
    }

    #[test]
    fn detect_os_family_matches_cfg() {
        let expected = if cfg!(target_os = "windows") {
            OsFamily::Windows
        } else if cfg!(target_os = "macos") {
            OsFamily::MacOs
        } else if cfg!(target_os = "linux") {
            OsFamily::Linux
        } else {
            OsFamily::Other
        };
        assert_eq!(detect().os.family, expected);
    }

    #[test]
    fn detect_arch_matches_env_consts() {
        assert_eq!(detect().os.arch, std::env::consts::ARCH);
    }
}
