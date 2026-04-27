//! Hardware-detection cache (Fase 2.4).
//!
//! Cold NVML/Vulkan init in [`gpu::detect`](super::gpu::detect) costs hundreds
//! of ms on every app launch. This module persists the previous detection
//! result to a JSON file in the per-OS app data directory and serves it back
//! when the host's cheap-to-detect fingerprint (OS + CPU + RAM-rounded-to-GiB)
//! still matches. A `force=true` path triggers fresh detection and overwrites
//! the file.
//!
//! Stored separately from the SQLite DB on purpose: this is a single-record
//! derived cache, not user data. The user can delete `hardware_cache.json`
//! at any time to force a re-detect; the SQLite file (conversations,
//! settings) is untouched.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::gpu::{self, GpuBackend};
use super::selector::{self, BackendChoice};
use super::system::{self, SystemInfo};

/// Bumped when the on-disk format or any detection logic changes in a way
/// that should invalidate every existing cache.
const CACHE_VERSION: u32 = 1;

/// Wire type returned to the frontend. `from_cache` lets the UI show
/// "Cached / Fresh"; `fingerprint` and `detected_at` are kept for debug
/// surface. Distinct from [`CachedDetection`] (the on-disk shape) so the
/// file format can evolve independently.
#[derive(Debug, Clone, Serialize, Type)]
pub struct HardwareDetection {
    pub system: SystemInfo,
    pub gpu: GpuBackend,
    pub choice: BackendChoice,
    pub fingerprint: String,
    pub detected_at: String,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedDetection {
    version: u32,
    fingerprint: String,
    detected_at: String,
    system: SystemInfo,
    gpu: GpuBackend,
    choice: BackendChoice,
}

/// Returns the cached detection if the on-disk file exists, parses, has a
/// matching version, and matches the current host's fingerprint. Otherwise
/// runs a fresh detection, persists it, and returns the fresh result.
///
/// On a cache hit we return the **current** [`SystemInfo`] (already detected
/// to compute the fingerprint) rather than the cached one, so that
/// momentary fields like `memory.available_bytes` aren't stale in the UI.
/// The cached system snapshot is preserved on disk for debugging but isn't
/// served back. The cached `gpu` + `choice` (the expensive bits) are
/// served as-is, since they only depend on hardware identity that the
/// fingerprint already validated.
pub fn load_or_detect(cache_path: &Path) -> HardwareDetection {
    let current_system = system::detect();
    let current_fp = fingerprint(&current_system);

    if let Some(cached) = read_cache(cache_path) {
        if cached.fingerprint == current_fp {
            tracing::info!(fingerprint = %current_fp, "hardware cache hit");
            return HardwareDetection {
                system: current_system,
                gpu: cached.gpu,
                choice: cached.choice,
                fingerprint: cached.fingerprint,
                detected_at: cached.detected_at,
                from_cache: true,
            };
        }
        tracing::info!(
            cached_fp = %cached.fingerprint,
            current_fp = %current_fp,
            "hardware fingerprint mismatch; re-detecting",
        );
    }

    let (system, gpu, choice) = detect_fresh_with(current_system);
    persist_and_return(cache_path, system, gpu, choice, current_fp)
}

/// Forces a full detection regardless of cache state and overwrites the
/// cache file. Used by the "Re-detect hardware" button so the user can
/// trigger a refresh after swapping a GPU (which the fingerprint can't see).
pub fn force_redetect(cache_path: &Path) -> HardwareDetection {
    let system = system::detect();
    let fp = fingerprint(&system);
    let (system, gpu, choice) = detect_fresh_with(system);
    persist_and_return(cache_path, system, gpu, choice, fp)
}

fn detect_fresh_with(system: SystemInfo) -> (SystemInfo, GpuBackend, BackendChoice) {
    let gpu = gpu::detect();
    let choice = selector::select_backend(&system, &gpu);
    (system, gpu, choice)
}

fn persist_and_return(
    cache_path: &Path,
    system: SystemInfo,
    gpu: GpuBackend,
    choice: BackendChoice,
    fingerprint: String,
) -> HardwareDetection {
    let detected_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::new());

    let to_persist = CachedDetection {
        version: CACHE_VERSION,
        fingerprint: fingerprint.clone(),
        detected_at: detected_at.clone(),
        system: system.clone(),
        gpu: gpu.clone(),
        choice: choice.clone(),
    };

    if let Err(e) = write_cache_atomic(cache_path, &to_persist) {
        tracing::warn!(
            error = %e,
            path = %cache_path.display(),
            "failed to persist hardware cache (continuing without cache)",
        );
    } else {
        tracing::info!(path = %cache_path.display(), "hardware cache written");
    }

    HardwareDetection {
        system,
        gpu,
        choice,
        fingerprint,
        detected_at,
        from_cache: false,
    }
}

fn read_cache(path: &Path) -> Option<CachedDetection> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            tracing::debug!("hardware cache file missing");
            return None;
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to read hardware cache");
            return None;
        }
    };

    match serde_json::from_slice::<CachedDetection>(&bytes) {
        Ok(c) if c.version == CACHE_VERSION => Some(c),
        Ok(c) => {
            tracing::info!(
                cached_version = c.version,
                expected = CACHE_VERSION,
                "hardware cache version mismatch; ignoring",
            );
            None
        }
        Err(e) => {
            tracing::warn!(error = %e, "hardware cache failed to parse; ignoring");
            None
        }
    }
}

fn write_cache_atomic(path: &Path, cached: &CachedDetection) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let tmp_path = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp_path)?;
        let bytes = serde_json::to_vec_pretty(cached).map_err(io::Error::other)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// SHA-256 hex of a stable string built from inputs that change only when
/// the *machine* itself changes. GPU info is intentionally excluded so
/// validating the cache is free; bumping `CACHE_VERSION` invalidates all
/// caches when detection logic changes (handled separately in
/// [`read_cache`]).
fn fingerprint(sys: &SystemInfo) -> String {
    let total_gib = rounded_gib(sys.memory.total_bytes);
    let payload = format!(
        "os={:?}|arch={}|cpu_vendor={}|cpu_brand={}|phys={}|log={}|mem_gib={}",
        sys.os.family,
        sys.os.arch,
        sys.cpu.vendor,
        sys.cpu.brand,
        sys.cpu.physical_cores,
        sys.cpu.logical_cores,
        total_gib,
    );
    let digest = Sha256::digest(payload.as_bytes());
    to_hex(&digest)
}

fn rounded_gib(total_bytes: u64) -> u64 {
    const GIB: u64 = 1024 * 1024 * 1024;
    (total_bytes + GIB / 2) / GIB
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::gpu::ComputeCapability;
    use crate::hardware::system::{CpuFeatures, CpuInfo, MemoryInfo, OsFamily, OsInfo};
    use std::path::PathBuf;

    fn fake_system() -> SystemInfo {
        SystemInfo {
            os: OsInfo {
                family: OsFamily::Linux,
                version: Some("Test Linux 1.0".into()),
                arch: "x86_64".into(),
            },
            cpu: CpuInfo {
                vendor: "TestVendor".into(),
                brand: "Test CPU @ 3.0GHz".into(),
                physical_cores: 8,
                logical_cores: 16,
                features: CpuFeatures {
                    avx2: true,
                    avx512f: false,
                },
            },
            memory: MemoryInfo {
                total_bytes: 16 * 1024 * 1024 * 1024,
                available_bytes: 8 * 1024 * 1024 * 1024,
            },
        }
    }

    fn fake_gpu() -> GpuBackend {
        GpuBackend::Cuda {
            name: "Test GPU".into(),
            vram_mb: 8192,
            compute_capability: ComputeCapability { major: 8, minor: 6 },
            uuid: "GPU-test".into(),
        }
    }

    fn fake_choice() -> BackendChoice {
        selector::select_backend(&fake_system(), &fake_gpu())
    }

    fn write_cached(path: &PathBuf, c: &CachedDetection) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(path, serde_json::to_vec(c).unwrap()).unwrap();
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let s = fake_system();
        assert_eq!(fingerprint(&s), fingerprint(&s));
    }

    #[test]
    fn fingerprint_changes_with_cpu_brand() {
        let mut s = fake_system();
        let a = fingerprint(&s);
        s.cpu.brand = "Different brand".into();
        assert_ne!(a, fingerprint(&s));
    }

    #[test]
    fn fingerprint_changes_with_arch() {
        let mut s = fake_system();
        let a = fingerprint(&s);
        s.os.arch = "aarch64".into();
        assert_ne!(a, fingerprint(&s));
    }

    #[test]
    fn fingerprint_changes_with_total_memory_gib() {
        let mut s = fake_system();
        let a = fingerprint(&s);
        s.memory.total_bytes = 32 * 1024 * 1024 * 1024;
        assert_ne!(a, fingerprint(&s));
    }

    #[test]
    fn fingerprint_stable_across_small_memory_drift() {
        let mut s = fake_system();
        s.memory.total_bytes = 16 * 1024 * 1024 * 1024;
        let a = fingerprint(&s);
        // Drift by 50 MiB — well within the GiB rounding boundary.
        s.memory.total_bytes = 16 * 1024 * 1024 * 1024 - 50 * 1024 * 1024;
        assert_eq!(a, fingerprint(&s));
    }

    #[test]
    fn load_or_detect_writes_file_on_first_call() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hardware_cache.json");
        assert!(!path.exists());
        let det = load_or_detect(&path);
        assert!(!det.from_cache);
        assert!(path.exists());
    }

    #[test]
    fn load_or_detect_returns_cached_on_match() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hardware_cache.json");

        let first = load_or_detect(&path);
        assert!(!first.from_cache);

        let second = load_or_detect(&path);
        assert!(second.from_cache);
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.detected_at, second.detected_at);
    }

    #[test]
    fn load_or_detect_redetects_on_fingerprint_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hardware_cache.json");

        let stale = CachedDetection {
            version: CACHE_VERSION,
            fingerprint: "deadbeef".into(),
            detected_at: "2000-01-01T00:00:00Z".into(),
            system: fake_system(),
            gpu: fake_gpu(),
            choice: fake_choice(),
        };
        write_cached(&path, &stale);

        let det = load_or_detect(&path);
        assert!(
            !det.from_cache,
            "fingerprint mismatch should force re-detect"
        );
        assert_ne!(det.fingerprint, "deadbeef");
    }

    #[test]
    fn load_or_detect_redetects_on_version_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hardware_cache.json");

        let stale = CachedDetection {
            version: 0,
            fingerprint: fingerprint(&system::detect()),
            detected_at: "2000-01-01T00:00:00Z".into(),
            system: fake_system(),
            gpu: fake_gpu(),
            choice: fake_choice(),
        };
        write_cached(&path, &stale);

        let det = load_or_detect(&path);
        assert!(!det.from_cache, "version mismatch should force re-detect");
    }

    #[test]
    fn load_or_detect_redetects_on_corrupt_json() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hardware_cache.json");
        std::fs::write(&path, b"{not really json").unwrap();

        let det = load_or_detect(&path);
        assert!(!det.from_cache, "corrupt cache should force re-detect");
        let bytes = std::fs::read(&path).unwrap();
        let parsed: CachedDetection = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.version, CACHE_VERSION);
    }

    #[test]
    fn cache_hit_returns_current_system_not_cached_one() {
        // Cached system claims an absurd `available_bytes` (and a different
        // os.version) that no real machine would have. The cache should
        // still hit (fingerprint matches) but the served `system` must be
        // the *current* fresh detection, not the on-disk one — that's how
        // we keep momentary fields like free RAM up to date in the UI.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hardware_cache.json");

        let mut bogus_system = system::detect();
        bogus_system.memory.available_bytes = 1; // 1 byte free — clearly synthetic.
        bogus_system.os.version = Some("BOGUS-CACHED-VERSION".into());

        let stale = CachedDetection {
            version: CACHE_VERSION,
            fingerprint: fingerprint(&system::detect()), // valid fingerprint for current host
            detected_at: "2000-01-01T00:00:00Z".into(),
            system: bogus_system,
            gpu: fake_gpu(),
            choice: fake_choice(),
        };
        write_cached(&path, &stale);

        let det = load_or_detect(&path);
        assert!(
            det.from_cache,
            "fingerprint matches — should be a cache hit"
        );
        // The expensive bits come from cache:
        assert!(matches!(det.gpu, GpuBackend::Cuda { .. }));
        // But system is fresh, not the bogus cached one:
        assert!(
            det.system.memory.available_bytes > 1,
            "available_bytes should be a fresh reading, not cached 1"
        );
        assert_ne!(
            det.system.os.version.as_deref(),
            Some("BOGUS-CACHED-VERSION"),
            "system should come from current detect(), not the on-disk snapshot"
        );
    }

    #[test]
    fn force_redetect_overwrites_even_on_match() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("hardware_cache.json");

        let first = load_or_detect(&path);
        let forced = force_redetect(&path);
        assert!(!forced.from_cache);
        assert_eq!(first.fingerprint, forced.fingerprint);
        assert!(!forced.detected_at.is_empty());
    }

    #[test]
    fn write_cache_failure_does_not_panic() {
        let tmp = tempfile::tempdir().unwrap();
        // Place a regular file at `blocker`, then attempt to write the cache
        // *inside* it. `fs::create_dir_all` rejects with NotADirectory on every
        // OS, so persistence fails — the call must still return a detection.
        let blocker = tmp.path().join("blocker");
        std::fs::write(&blocker, b"").unwrap();
        let bad_path = blocker.join("hardware_cache.json");

        let det = load_or_detect(&bad_path);
        assert!(!det.from_cache);
    }
}
