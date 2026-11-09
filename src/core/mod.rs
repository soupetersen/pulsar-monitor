//! Data collection, independent of any UI toolkit: the sampler, the
//! per-OS sources (`platform`), the NVIDIA library and, on Windows, the
//! CPU temperature helper. The UI starts the sampler with a callback that
//! receives a `Stats` once per second, and tells it what's on screen
//! through `SharedView`.

#[cfg(windows)]
pub mod cpu_sensor;
mod gpu;
#[cfg(windows)]
mod gpu_kmt;
#[cfg(windows)]
mod pdh;
mod platform;
mod sampler;

#[cfg(target_os = "linux")]
pub use platform::{enable_power, power_setup_needed};
pub use sampler::{Panel, ProcStats, RamStats, SharedView, Stats, View, spawn};

/// `--gpu-info`: prints what each GPU reports (diagnostics).
pub fn print_gpu_info() {
    #[cfg(windows)]
    {
        for a in gpu_kmt::adapters() {
            println!(
                "D3DKMT  {:<40} luid {}  vram {:.1} GB  {:?}",
                a.name,
                a.luid,
                a.vram_total as f64 / (1u64 << 30) as f64,
                a.perf()
            );
        }
        if let Some(main) = gpu_kmt::main_adapter() {
            println!("main adapter: {}", main.name);
        }
    }
    match gpu::NvGpu::new().and_then(|nv| nv.sample().map(|s| (nv.name.clone(), s))) {
        Some((name, s)) => println!(
            "NVML    {name:<40} usage {}%  temp {:?}  power {:?} W  fan {:?}%",
            s.usage, s.temp_c, s.power_w, s.fan_pct
        ),
        None => println!("NVML    not available"),
    }
}
