//! OS-specific data. The common sampler (`sampler.rs`) reads everything
//! `sysinfo` and NVML offer on every platform; each platform module adds what
//! only its OS can provide (effective clock, CPU power and temperature, disk
//! activity, non-NVIDIA GPUs, per-process GPU usage).

use std::collections::HashMap;

use crate::sampler::{DiskStats, GpuStats};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::Platform;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use self::linux::Platform;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::Platform;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod other;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub use self::other::Platform;

/// What the UI is showing, so platforms only collect what's needed.
#[derive(Clone, Copy)]
pub struct Want {
    pub expanded: bool,
    pub gpu_panel: bool,
    /// The GPU is read through NVML; the platform shouldn't report one.
    pub nvml: bool,
}

/// Per-process GPU usage (%) and dedicated memory (bytes), by PID.
#[derive(Default)]
pub struct GpuProcs {
    pub usage: HashMap<u32, f32>,
    pub vram: HashMap<u32, u64>,
}

#[derive(Default)]
pub struct Extras {
    pub clock_ghz: Option<f32>,
    pub cpu_power_w: Option<f32>,
    pub cpu_temp_c: Option<f32>,
    pub disk: Option<DiskStats>,
    /// The main GPU when it isn't read through NVML.
    pub gpu: Option<GpuStats>,
    pub gpu_procs: Option<GpuProcs>,
}
