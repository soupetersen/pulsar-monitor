//! macOS: temperatures from the system sensors `sysinfo` exposes, and GPU
//! usage from the IOKit `IOAccelerator` performance statistics (Apple
//! Silicon and Intel Macs). No NVIDIA/NVML on current macOS.

use core_foundation::base::{kCFAllocatorDefault, CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFMutableDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use io_kit_sys::types::io_iterator_t;
use io_kit_sys::{
    kIOMasterPortDefault, IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperties, IOServiceGetMatchingServices,
    IOServiceMatching,
};
use sysinfo::Components;

use super::{Extras, Want};
use crate::sampler::GpuStats;

pub struct Platform {
    components: Option<Components>,
}

impl Platform {
    pub fn new() -> Self {
        Self { components: Components::new_with_refreshed_list().ok() }
    }

    pub fn wants_nvml(&self) -> bool {
        false
    }

    pub fn sample(&mut self, _want: Want) -> Extras {
        let mut extras = Extras::default();
        if let Some(components) = &mut self.components {
            components.refresh(false);
            extras.cpu_temp_c = cpu_temp(components);
        }
        extras.gpu = gpu_sample();
        extras
    }

    pub fn on_hidden(&mut self) {}
}

/// Hottest CPU die sensor: "PMU tdie*" on Apple Silicon, "CPU ..." on Intel Macs.
fn cpu_temp(components: &Components) -> Option<f32> {
    components
        .iter()
        .filter(|c| {
            let label = c.label().to_lowercase();
            label.contains("tdie") || label.contains("cpu")
        })
        .filter_map(|c| c.temperature())
        .filter(|t| t.is_finite() && *t > 0.0)
        .reduce(f32::max)
}

/// The busiest GPU reported by IOKit.
fn gpu_sample() -> Option<GpuStats> {
    unsafe {
        let matching = IOServiceMatching(c"IOAccelerator".as_ptr());
        if matching.is_null() {
            return None;
        }
        let mut iter: io_iterator_t = 0;
        // Consumes `matching`.
        if IOServiceGetMatchingServices(kIOMasterPortDefault, matching as _, &mut iter) != 0 {
            return None;
        }
        let key = |k: &'static str| CFString::from_static_string(k);
        let mut best: Option<GpuStats> = None;
        loop {
            let service = IOIteratorNext(iter);
            if service == 0 {
                break;
            }
            let mut props: CFMutableDictionaryRef = std::ptr::null_mut();
            if IORegistryEntryCreateCFProperties(service, &mut props, kCFAllocatorDefault, 0) == 0 && !props.is_null() {
                let dict: CFDictionary<CFString, CFType> = CFDictionary::wrap_under_create_rule(props as _);
                if let Some(stats) = dict.find(key("PerformanceStatistics")) {
                    let stats: CFDictionary<CFString, CFType> = CFDictionary::wrap_under_get_rule(stats.as_CFTypeRef() as _);
                    let number =
                        |k: &'static str| stats.find(key(k)).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_i64());
                    if let Some(usage) = number("Device Utilization %").or_else(|| number("GPU Activity(%)")) {
                        let name = dict
                            .find(key("model"))
                            .and_then(|v| v.downcast::<CFString>())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "Apple GPU".into());
                        let gpu = GpuStats {
                            name,
                            usage: usage as f32,
                            vram_used: number("In use system memory").map(|b| b as u64),
                            ..Default::default()
                        };
                        if best.as_ref().is_none_or(|b| gpu.usage >= b.usage) {
                            best = Some(gpu);
                        }
                    }
                }
            }
            IOObjectRelease(service);
        }
        IOObjectRelease(iter);
        best
    }
}
