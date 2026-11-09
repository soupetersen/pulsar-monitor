//! Background sampler: collects system stats once per second and emits them
//! to the UI. It only gathers what the current view shows, so the compact
//! widget stays cheap and a hidden widget costs nothing.
//!
//! Everything `sysinfo` and NVML provide is read here on every platform;
//! OS-specific extras come from `crate::platform`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sysinfo::{Networks, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};

use crate::gpu::NvGpu;
use crate::platform::{GpuProcs, Platform, Want};

const TICK: Duration = Duration::from_secs(1);
const FIRST_TICK: Duration = Duration::from_millis(300);
const TOP_PROCS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Panel {
    None,
    Cpu,
    Gpu,
    Ram,
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub visible: bool,
    pub expanded: bool,
    pub panel: Panel,
}

pub type SharedView = Arc<Mutex<View>>;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Stats {
    host: String,
    uptime_secs: u64,
    cpu: CpuStats,
    ram: RamStats,
    gpu: Option<GpuStats>,
    net: Option<NetStats>,
    disk: Option<DiskStats>,
    procs: Option<Vec<ProcStats>>,
    procs_for: Option<Panel>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CpuStats {
    name: String,
    usage: f32,
    clock_ghz: Option<f32>,
    temp_c: Option<f32>,
    power_w: Option<f32>,
    threads: usize,
    cores: Vec<f32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RamStats {
    used: u64,
    total: u64,
    available: u64,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuStats {
    pub name: String,
    pub usage: f32,
    pub temp_c: Option<u32>,
    pub power_w: Option<f32>,
    pub power_limit_w: Option<f32>,
    /// Power as a percentage of the adapter's maximum (when watts aren't known).
    pub power_pct: Option<f32>,
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub fan_pct: Option<u32>,
    /// Fan speed in RPM (when the percentage isn't known).
    pub fan_rpm: Option<u32>,
    pub clock_mhz: Option<u32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NetStats {
    rx_per_sec: f64,
    tx_per_sec: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiskStats {
    pub active_pct: f64,
    pub bytes_per_sec: f64,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct ProcStats {
    name: String,
    count: u32,
    cpu: f32,
    mem: u64,
    gpu: f32,
    vram: u64,
}

pub fn spawn(app: AppHandle, view: SharedView) {
    std::thread::Builder::new()
        .name("sampler".into())
        .spawn(move || run(app, view))
        .expect("failed to start sampler thread");
}

fn run(app: AppHandle, view: SharedView) {
    let Ok(mut sys) = System::new() else { return };
    sys.refresh_cpu_all();
    let cpu_name = sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
    let ncpu = sys.cpus().len().max(1) as f32;
    let host = System::host_name().unwrap_or_default();

    let mut platform = Platform::new();
    let nv = if platform.wants_nvml() { NvGpu::new() } else { None };
    let mut networks = Networks::new_with_refreshed_list().ok();
    let mut last_net = Instant::now();

    // First sample soon after startup so the UI isn't left loading for a full
    // second; CPU usage only needs a couple hundred ms between refreshes.
    let mut next = Instant::now() + FIRST_TICK;
    loop {
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
        next += TICK;
        if next < Instant::now() {
            next = Instant::now() + TICK;
        }

        let v = *view.lock().unwrap();
        if !v.visible {
            platform.on_hidden();
            continue;
        }

        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let nv_sample = nv.as_ref().and_then(NvGpu::sample);
        let extras = platform.sample(Want {
            expanded: v.expanded,
            gpu_panel: v.expanded && v.panel == Panel::Gpu,
            nvml: nv_sample.is_some(),
        });

        let gpu = match (&nv, nv_sample) {
            (Some(nv), Some(s)) => Some(GpuStats {
                name: nv.name.clone(),
                usage: s.usage,
                temp_c: s.temp_c,
                power_w: s.power_w,
                power_limit_w: s.power_limit_w,
                vram_used: Some(s.vram_used),
                vram_total: Some(s.vram_total),
                fan_pct: s.fan_pct,
                clock_mhz: s.clock_mhz,
                ..Default::default()
            }),
            _ => extras.gpu,
        };

        let net = if v.expanded {
            networks.as_mut().map(|n| {
                n.refresh(true);
                let secs = last_net.elapsed().as_secs_f64().max(0.001);
                last_net = Instant::now();
                let (rx, tx) = n.values().fold((0u64, 0u64), |(r, t), d| (r + d.received(), t + d.transmitted()));
                NetStats { rx_per_sec: rx as f64 / secs, tx_per_sec: tx as f64 / secs }
            })
        } else {
            None
        };

        let procs = if v.expanded && v.panel != Panel::None {
            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );
            Some(top_processes(&sys, v.panel, ncpu, extras.gpu_procs.as_ref()))
        } else {
            None
        };

        let stats = Stats {
            host: host.clone(),
            uptime_secs: System::uptime().unwrap_or(0),
            cpu: CpuStats {
                name: cpu_name.clone(),
                usage: sys.global_cpu_usage(),
                clock_ghz: extras.clock_ghz,
                temp_c: extras.cpu_temp_c,
                power_w: extras.cpu_power_w,
                threads: sys.cpus().len(),
                cores: if v.expanded { sys.cpus().iter().map(|c| c.usage()).collect() } else { Vec::new() },
            },
            ram: RamStats {
                used: sys.used_memory(),
                total: sys.total_memory(),
                available: sys.available_memory(),
            },
            gpu,
            net,
            disk: extras.disk,
            procs_for: procs.as_ref().map(|_| v.panel),
            procs,
        };
        let _ = app.emit("stats", stats);
    }
}

/// Groups processes by executable name (e.g. all chrome.exe together) and
/// returns the top entries for the open panel.
fn top_processes(sys: &System, panel: Panel, ncpu: f32, gpu: Option<&GpuProcs>) -> Vec<ProcStats> {
    let mut groups: HashMap<String, ProcStats> = HashMap::new();
    for (pid, p) in sys.processes() {
        let pid = pid.as_u32();
        if pid == 0 {
            continue;
        }
        let name = p.name().to_string_lossy().into_owned();
        let g = groups.entry(name.clone()).or_insert_with(|| ProcStats { name, ..Default::default() });
        g.count += 1;
        g.cpu += p.cpu_usage() / ncpu;
        g.mem += p.memory();
        if let Some(gp) = gpu {
            g.gpu += gp.usage.get(&pid).copied().unwrap_or(0.0);
            g.vram += gp.vram.get(&pid).copied().unwrap_or(0);
        }
    }
    let mut list: Vec<ProcStats> = groups.into_values().collect();
    match panel {
        Panel::Gpu => {
            list.retain(|p| p.gpu > 0.0 || p.vram > 0);
            list.sort_by(|a, b| b.gpu.total_cmp(&a.gpu).then(b.vram.cmp(&a.vram)));
        }
        Panel::Ram => list.sort_by(|a, b| b.mem.cmp(&a.mem)),
        _ => list.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
    }
    list.truncate(TOP_PROCS);
    list
}
