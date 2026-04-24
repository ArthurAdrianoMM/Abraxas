//! Vulkan physical-device probe (Windows + Linux).
//!
//! Uses `ash` — raw Vulkan FFI — because we only need basic enumeration
//! (physical devices, properties, memory heaps). `vulkano` would pull in
//! shaderc + rendering pipeline machinery we never touch.
//!
//! Loader handling: `Entry::load()` dynamically opens `libvulkan.so.1` /
//! `vulkan-1.dll`. On ubuntu-latest CI runners the loader isn't installed by
//! default, so this returns `None` cleanly — selector tests mustn't assume a
//! specific backend. Fase 3.4's CI will install `libvulkan1` when needed.
//!
//! Fase 2.3 hand-off: if NVML fails but Vulkan sees an NVIDIA device (e.g.,
//! Linux with `nouveau`, no CUDA runtime), we return `Vulkan { vendor: Nvidia }`.
//! That's correct — CUDA truly isn't available — so the selector MUST key off
//! the `GpuBackend::Cuda` discriminant, not the `VulkanVendor`.

use std::ffi::{CStr, CString};

use ash::{vk, Entry, Instance};

use super::{GpuBackend, VulkanDeviceType, VulkanVendor};

const VENDOR_AMD: u32 = 0x1002;
const VENDOR_INTEL: u32 = 0x8086;
const VENDOR_NVIDIA: u32 = 0x10DE;

pub fn probe() -> Option<GpuBackend> {
    let entry = match unsafe { Entry::load() } {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "vulkan loader not available");
            return None;
        }
    };

    let app_name = CString::new("Abraxas").ok()?;
    let app_info = vk::ApplicationInfo::default()
        .application_name(&app_name)
        .api_version(vk::API_VERSION_1_0);
    let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);

    let instance = match unsafe { entry.create_instance(&create_info, None) } {
        Ok(i) => InstanceGuard(i),
        Err(e) => {
            tracing::warn!(error = %e, "vulkan create_instance failed (no ICD?)");
            return None;
        }
    };

    let devices = match unsafe { instance.0.enumerate_physical_devices() } {
        Ok(d) if d.is_empty() => {
            tracing::info!("vulkan reports 0 physical devices");
            return None;
        }
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, "vulkan enumerate_physical_devices failed");
            return None;
        }
    };

    let mut best: Option<PickedDevice> = None;
    for device in devices {
        let props = unsafe { instance.0.get_physical_device_properties(device) };
        let mem_props = unsafe { instance.0.get_physical_device_memory_properties(device) };

        let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let vendor_id = props.vendor_id;
        let vendor = vendor_from_id(vendor_id);
        let device_type = device_type_from_vk(props.device_type);
        let vram_mb = device_local_vram_mb(&mem_props);

        let candidate = PickedDevice {
            tier: tier_for(props.device_type),
            vram_mb,
            name,
            vendor,
            vendor_id,
            device_type,
        };

        if best
            .as_ref()
            .map(|b| candidate.better_than(b))
            .unwrap_or(true)
        {
            best = Some(candidate);
        }
    }

    best.map(|p| GpuBackend::Vulkan {
        vendor: p.vendor,
        vendor_id: p.vendor_id,
        name: p.name,
        vram_mb: p.vram_mb,
        device_type: p.device_type,
    })
}

struct InstanceGuard(Instance);
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe { self.0.destroy_instance(None) };
    }
}

struct PickedDevice {
    tier: u8,
    vram_mb: Option<u64>,
    name: String,
    vendor: VulkanVendor,
    vendor_id: u32,
    device_type: VulkanDeviceType,
}

impl PickedDevice {
    fn better_than(&self, other: &Self) -> bool {
        if self.tier != other.tier {
            return self.tier < other.tier;
        }
        self.vram_mb.unwrap_or(0) > other.vram_mb.unwrap_or(0)
    }
}

fn vendor_from_id(id: u32) -> VulkanVendor {
    match id {
        VENDOR_AMD => VulkanVendor::Amd,
        VENDOR_INTEL => VulkanVendor::Intel,
        VENDOR_NVIDIA => VulkanVendor::Nvidia,
        _ => VulkanVendor::Other,
    }
}

fn device_type_from_vk(t: vk::PhysicalDeviceType) -> VulkanDeviceType {
    match t {
        vk::PhysicalDeviceType::DISCRETE_GPU => VulkanDeviceType::Discrete,
        vk::PhysicalDeviceType::INTEGRATED_GPU => VulkanDeviceType::Integrated,
        vk::PhysicalDeviceType::VIRTUAL_GPU => VulkanDeviceType::Virtual,
        vk::PhysicalDeviceType::CPU => VulkanDeviceType::Cpu,
        _ => VulkanDeviceType::Other,
    }
}

/// Preference ordering: Discrete > Integrated > Virtual/Cpu/Other. Lower tier
/// wins. Must align with `device_type_from_vk` but is indexed off the raw
/// `vk::PhysicalDeviceType` to keep this a pure branch-free decision.
fn tier_for(t: vk::PhysicalDeviceType) -> u8 {
    match t {
        vk::PhysicalDeviceType::DISCRETE_GPU => 0,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
        _ => 2,
    }
}

fn device_local_vram_mb(props: &vk::PhysicalDeviceMemoryProperties) -> Option<u64> {
    let count = props.memory_heap_count as usize;
    let total: u64 = props.memory_heaps[..count]
        .iter()
        .filter(|h| {
            h.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL)
                && !h.flags.contains(vk::MemoryHeapFlags::MULTI_INSTANCE)
        })
        .map(|h| h.size)
        .sum();
    if total == 0 {
        None
    } else {
        Some(total / 1_048_576)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_id_mapping_covers_known_vendors() {
        assert_eq!(vendor_from_id(VENDOR_AMD), VulkanVendor::Amd);
        assert_eq!(vendor_from_id(VENDOR_INTEL), VulkanVendor::Intel);
        assert_eq!(vendor_from_id(VENDOR_NVIDIA), VulkanVendor::Nvidia);
    }

    #[test]
    fn vendor_id_mapping_falls_back_to_other() {
        assert_eq!(vendor_from_id(0x0000), VulkanVendor::Other);
        assert_eq!(vendor_from_id(0x13B5), VulkanVendor::Other); // ARM Mali
        assert_eq!(vendor_from_id(0xFFFF), VulkanVendor::Other);
    }

    #[test]
    fn device_type_mapping_is_exhaustive_for_known_values() {
        assert_eq!(
            device_type_from_vk(vk::PhysicalDeviceType::DISCRETE_GPU),
            VulkanDeviceType::Discrete,
        );
        assert_eq!(
            device_type_from_vk(vk::PhysicalDeviceType::INTEGRATED_GPU),
            VulkanDeviceType::Integrated,
        );
        assert_eq!(
            device_type_from_vk(vk::PhysicalDeviceType::VIRTUAL_GPU),
            VulkanDeviceType::Virtual,
        );
        assert_eq!(
            device_type_from_vk(vk::PhysicalDeviceType::CPU),
            VulkanDeviceType::Cpu,
        );
        assert_eq!(
            device_type_from_vk(vk::PhysicalDeviceType::OTHER),
            VulkanDeviceType::Other,
        );
    }

    #[test]
    fn tier_prefers_discrete_then_integrated() {
        assert!(
            tier_for(vk::PhysicalDeviceType::DISCRETE_GPU)
                < tier_for(vk::PhysicalDeviceType::INTEGRATED_GPU)
        );
        assert!(
            tier_for(vk::PhysicalDeviceType::INTEGRATED_GPU)
                < tier_for(vk::PhysicalDeviceType::CPU)
        );
        assert!(
            tier_for(vk::PhysicalDeviceType::INTEGRATED_GPU)
                < tier_for(vk::PhysicalDeviceType::VIRTUAL_GPU)
        );
    }
}
