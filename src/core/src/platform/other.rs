//! Platforms without their own module yet: only what `sysinfo` and NVML
//! provide in the common sampler.

use super::{Extras, Want};

pub struct Platform;

impl Platform {
    pub fn new() -> Self {
        Self
    }

    /// Whether the GPU should be read through NVML when it's available.
    pub fn wants_nvml(&self) -> bool {
        true
    }

    pub fn sample(&mut self, _want: Want) -> Extras {
        Extras::default()
    }

    /// The widget was hidden: release anything expensive.
    pub fn on_hidden(&mut self) {}
}
