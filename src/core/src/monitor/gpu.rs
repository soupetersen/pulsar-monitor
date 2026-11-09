//! NVIDIA GPU stats through NVML (nvml.dll ships with the NVIDIA driver).

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};

pub struct NvGpu {
    nvml: Nvml,
    pub name: String,
}

pub struct NvSample {
    pub usage: f32,
    pub temp_c: Option<u32>,
    pub power_w: Option<f32>,
    pub power_limit_w: Option<f32>,
    pub vram_used: u64,
    pub vram_total: u64,
    pub fan_pct: Option<u32>,
    pub clock_mhz: Option<u32>,
}

impl NvGpu {
    pub fn new() -> Option<Self> {
        let nvml = Nvml::init().ok()?;
        let name = nvml.device_by_index(0).ok()?.name().ok()?;
        let name = name.trim_start_matches("NVIDIA ").trim_start_matches("GeForce ").to_string();
        Some(Self { nvml, name })
    }

    pub fn sample(&self) -> Option<NvSample> {
        let dev = self.nvml.device_by_index(0).ok()?;
        let mem = dev.memory_info().ok()?;
        Some(NvSample {
            usage: dev.utilization_rates().ok()?.gpu as f32,
            temp_c: dev.temperature(TemperatureSensor::Gpu).ok(),
            power_w: dev.power_usage().ok().map(|mw| mw as f32 / 1000.0),
            power_limit_w: dev.enforced_power_limit().ok().map(|mw| mw as f32 / 1000.0),
            vram_used: mem.used,
            vram_total: mem.total,
            fan_pct: dev.fan_speed(0).ok(),
            clock_mhz: dev.clock_info(Clock::Graphics).ok(),
        })
    }
}
