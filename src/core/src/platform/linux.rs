//! Linux: everything comes from sysfs/procfs and works without root, except
//! the RAPL energy counter, which many distributions restrict to root (then
//! the CPU power just isn't shown).
//!
//! - CPU temperature: hwmon (`k10temp`/`zenpower` on AMD, `coretemp` on Intel)
//! - CPU power: RAPL through `/sys/class/powercap`
//! - Effective clock: average of `scaling_cur_freq`
//! - Disk activity: `/proc/diskstats`
//! - AMD GPU: `amdgpu` sysfs (usage, VRAM, temperature, power, fan)

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::{Extras, Want};
use crate::sampler::{DiskStats, GpuStats};

pub struct Platform {
    cpu_temp: Option<PathBuf>,
    rapl: Option<Rapl>,
    disks: DiskCounter,
    amd_gpu: Option<AmdGpu>,
}

impl Platform {
    pub fn new() -> Self {
        Self { cpu_temp: find_cpu_temp(), rapl: Rapl::new(), disks: DiskCounter::default(), amd_gpu: AmdGpu::find() }
    }

    /// NVML when an NVIDIA GPU is there; otherwise the AMD GPU from sysfs.
    pub fn wants_nvml(&self) -> bool {
        true
    }

    pub fn sample(&mut self, want: Want) -> Extras {
        let mut extras = Extras {
            clock_ghz: average_clock_ghz(),
            cpu_power_w: self.rapl.as_mut().and_then(Rapl::watts),
            cpu_temp_c: self.cpu_temp.as_deref().and_then(read_millis).map(|m| m as f32 / 1000.0),
            ..Default::default()
        };
        if want.expanded {
            extras.disk = self.disks.sample();
        }
        if !want.nvml {
            extras.gpu = self.amd_gpu.as_ref().map(AmdGpu::sample);
        }
        extras
    }

    pub fn on_hidden(&mut self) {}
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_millis(path: &Path) -> Option<i64> {
    read_trimmed(path)?.parse().ok()
}

fn hwmons() -> Vec<PathBuf> {
    fs::read_dir("/sys/class/hwmon").map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default()
}

/// The hwmon input that best represents the CPU package temperature.
fn find_cpu_temp() -> Option<PathBuf> {
    for hwmon in hwmons() {
        let name = read_trimmed(&hwmon.join("name")).unwrap_or_default();
        // Preferred label per driver; falls back to temp1.
        let wanted: &[&str] = match name.as_str() {
            "k10temp" => &["Tctl", "Tdie"],
            "zenpower" => &["Tdie", "Tctl"],
            "coretemp" => &["Package id 0"],
            "cpu_thermal" | "soc_thermal" => &[],
            _ => continue,
        };
        for label in wanted {
            for i in 1..=32 {
                if read_trimmed(&hwmon.join(format!("temp{i}_label"))).as_deref() == Some(label) {
                    return Some(hwmon.join(format!("temp{i}_input")));
                }
            }
        }
        let first = hwmon.join("temp1_input");
        if first.exists() {
            return Some(first);
        }
    }
    None
}

/// Package energy counter (µJ); power is its rate of change.
struct Rapl {
    energy: PathBuf,
    max: u64,
    last: Option<(u64, Instant)>,
}

impl Rapl {
    fn new() -> Option<Self> {
        let dir = PathBuf::from("/sys/class/powercap/intel-rapl:0");
        let energy = dir.join("energy_uj");
        // Unreadable without root on many distributions.
        fs::read_to_string(&energy).ok()?;
        let max = read_millis(&dir.join("max_energy_range_uj")).unwrap_or(i64::MAX) as u64;
        Some(Self { energy, max, last: None })
    }

    fn watts(&mut self) -> Option<f32> {
        let now = Instant::now();
        let uj = read_millis(&self.energy)? as u64;
        let watts = self.last.map(|(prev, at)| {
            let delta = if uj >= prev { uj - prev } else { self.max - prev + uj };
            delta as f64 / 1e6 / now.duration_since(at).as_secs_f64().max(0.001)
        });
        self.last = Some((uj, now));
        watts.map(|w| w as f32)
    }
}

fn average_clock_ghz() -> Option<f32> {
    let dirs = fs::read_dir("/sys/devices/system/cpu").ok()?;
    let khz: Vec<f64> = dirs
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().strip_prefix("cpu").is_some_and(|n| n.chars().all(|c| c.is_ascii_digit())))
        .filter_map(|e| read_millis(&e.path().join("cpufreq/scaling_cur_freq")))
        .map(|k| k as f64)
        .collect();
    (!khz.is_empty()).then(|| (khz.iter().sum::<f64>() / khz.len() as f64 / 1e6) as f32)
}

/// Whole physical disks from /proc/diskstats (partitions and virtual devices skipped).
#[derive(Default)]
struct DiskCounter {
    /// (bytes transferred, milliseconds busy per disk, when)
    last: Option<(u64, Vec<(String, u64)>, Instant)>,
}

fn is_whole_disk(name: &str) -> bool {
    let nvme = name.starts_with("nvme") && name.contains('n') && !name.contains('p');
    let sd = (name.starts_with("sd") || name.starts_with("vd") || name.starts_with("hd"))
        && !name.chars().last().is_some_and(|c| c.is_ascii_digit());
    let mmc = name.starts_with("mmcblk") && !name.contains('p');
    nvme || sd || mmc
}

impl DiskCounter {
    fn sample(&mut self) -> Option<DiskStats> {
        let text = fs::read_to_string("/proc/diskstats").ok()?;
        let mut bytes = 0u64;
        let mut busy = Vec::new();
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 13 || !is_whole_disk(f[2]) {
                continue;
            }
            let num = |i: usize| f[i].parse::<u64>().unwrap_or(0);
            // Fields: 5 sectors read, 9 sectors written, 12 ms spent doing I/O.
            bytes += (num(5) + num(9)) * 512;
            busy.push((f[2].to_string(), num(12)));
        }
        let now = Instant::now();
        let stats = self.last.as_ref().map(|(prev_bytes, prev_busy, at)| {
            let ms = now.duration_since(*at).as_secs_f64() * 1000.0;
            let active = busy
                .iter()
                .filter_map(|(name, b)| prev_busy.iter().find(|(n, _)| n == name).map(|(_, p)| b.saturating_sub(*p)))
                .map(|d| d as f64 / ms.max(1.0) * 100.0)
                .fold(0.0, f64::max);
            DiskStats { active_pct: active.min(100.0), bytes_per_sec: bytes.saturating_sub(*prev_bytes) as f64 / (ms / 1000.0).max(0.001) }
        });
        self.last = Some((bytes, busy, now));
        stats
    }
}

/// AMD GPU through the amdgpu driver's sysfs files.
struct AmdGpu {
    device: PathBuf,
    hwmon: Option<PathBuf>,
    name: String,
}

impl AmdGpu {
    /// The AMD card with the most VRAM (discrete over integrated).
    fn find() -> Option<Self> {
        let cards = fs::read_dir("/sys/class/drm").ok()?;
        let mut best: Option<(u64, PathBuf)> = None;
        for card in cards.flatten() {
            let name = card.file_name().to_string_lossy().to_string();
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }
            let device = card.path().join("device");
            if read_trimmed(&device.join("vendor")).as_deref() != Some("0x1002") {
                continue;
            }
            let vram = read_millis(&device.join("mem_info_vram_total")).unwrap_or(0) as u64;
            if best.as_ref().is_none_or(|(v, _)| vram > *v) {
                best = Some((vram, device));
            }
        }
        let (_, device) = best?;
        let hwmon = fs::read_dir(device.join("hwmon")).ok().and_then(|mut d| d.next()).and_then(Result::ok).map(|e| e.path());
        let name = read_trimmed(&device.join("product_name")).filter(|n| !n.is_empty()).unwrap_or_else(|| "AMD Radeon".into());
        Some(Self { device, hwmon, name })
    }

    fn sample(&self) -> GpuStats {
        let dev = |f: &str| read_millis(&self.device.join(f));
        let hw = |f: &str| self.hwmon.as_ref().and_then(|h| read_millis(&h.join(f)));
        // power1_average on most cards, power1_input on newer ones (µW).
        let power_uw = hw("power1_average").or_else(|| hw("power1_input"));
        let fan_pct = match (hw("pwm1"), hw("pwm1_max")) {
            (Some(pwm), max) => Some((pwm as f32 / max.unwrap_or(255).max(1) as f32 * 100.0).round() as u32),
            _ => None,
        };
        GpuStats {
            name: self.name.clone(),
            usage: dev("gpu_busy_percent").unwrap_or(0) as f32,
            temp_c: hw("temp1_input").map(|m| (m as f32 / 1000.0).round() as u32),
            power_w: power_uw.map(|uw| uw as f32 / 1e6),
            power_limit_w: hw("power1_cap").map(|uw| uw as f32 / 1e6),
            vram_used: dev("mem_info_vram_used").map(|b| b as u64),
            vram_total: dev("mem_info_vram_total").map(|b| b as u64),
            fan_pct,
            fan_rpm: hw("fan1_input").map(|r| r as u32),
            clock_mhz: hw("freq1_input").map(|hz| (hz / 1_000_000) as u32),
            ..Default::default()
        }
    }
}
