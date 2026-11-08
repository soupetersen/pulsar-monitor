//! Background sampler: collects system stats once per second and emits them
//! to the UI. It only gathers what the current view shows, so the compact
//! widget stays cheap and a hidden widget costs nothing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sysinfo::{Networks, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};

use crate::gpu::NvGpu;
use crate::pdh::{Counter, Query};

const TICK: Duration = Duration::from_secs(1);
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

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GpuStats {
    name: String,
    usage: f32,
    temp_c: Option<u32>,
    power_w: Option<f32>,
    power_limit_w: Option<f32>,
    vram_used: Option<u64>,
    vram_total: Option<u64>,
    fan_pct: Option<u32>,
    clock_mhz: Option<u32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NetStats {
    rx_per_sec: f64,
    tx_per_sec: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DiskStats {
    active_pct: f64,
    bytes_per_sec: f64,
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

/// Always-on counters (cheap).
struct BaseCounters {
    query: Query,
    cpu_perf: Option<Counter>,
    cpu_freq: Option<Counter>,
    /// CPU package power from RAPL, in mW (Windows "Energy Meter" counters).
    cpu_power: Option<Counter>,
    disk_idle: Option<Counter>,
    disk_bytes: Option<Counter>,
}

impl BaseCounters {
    fn new() -> Option<Self> {
        let query = Query::new()?;
        let cpu_perf = query.add(r"\Processor Information(_Total)\% Processor Performance");
        let cpu_freq = query.add(r"\Processor Information(_Total)\Processor Frequency");
        let cpu_power = query.add(r"\Energy Meter(rapl_package0_pkg)\Power");
        let disk_idle = query.add(r"\PhysicalDisk(_Total)\% Idle Time");
        let disk_bytes = query.add(r"\PhysicalDisk(_Total)\Disk Bytes/sec");
        query.collect();
        Some(Self { query, cpu_perf, cpu_freq, cpu_power, disk_idle, disk_bytes })
    }
}

/// Per-process GPU counters. Enumerating GPU engines is relatively expensive,
/// so this only exists while the GPU panel is open (or when there is no NVML).
struct GpuCounters {
    query: Query,
    engine: Counter,
    memory: Option<Counter>,
}

impl GpuCounters {
    fn new() -> Option<Self> {
        let query = Query::new()?;
        let engine = query.add(r"\GPU Engine(*)\Utilization Percentage")?;
        let memory = query.add(r"\GPU Process Memory(*)\Dedicated Usage");
        query.collect();
        Some(Self { query, engine, memory })
    }

    /// Returns (per-pid GPU %, per-pid dedicated VRAM bytes, overall GPU % estimate).
    fn sample(&self) -> (HashMap<u32, f32>, HashMap<u32, u64>, f32) {
        self.query.collect();

        // Sum engine instances per (pid, adapter, engine type), then take the
        // busiest engine type per pid, like Task Manager's "GPU" column.
        let mut per_engine: HashMap<(u32, String, String), f64> = HashMap::new();
        for (name, value) in self.engine.array() {
            if let Some((pid, luid, engtype)) = parse_engine_instance(&name) {
                *per_engine.entry((pid, luid, engtype)).or_default() += value;
            }
        }
        let mut per_pid: HashMap<u32, f32> = HashMap::new();
        let mut per_type: HashMap<(String, String), f64> = HashMap::new();
        for ((pid, luid, engtype), value) in per_engine {
            let e = per_pid.entry(pid).or_default();
            *e = e.max(value as f32);
            *per_type.entry((luid, engtype)).or_default() += value;
        }
        let overall = per_type.values().cloned().fold(0.0, f64::max).min(100.0) as f32;

        let mut vram: HashMap<u32, u64> = HashMap::new();
        if let Some(mem) = &self.memory {
            for (name, value) in mem.array() {
                if let Some(pid) = parse_pid(&name) {
                    *vram.entry(pid).or_default() += value as u64;
                }
            }
        }
        (per_pid, vram, overall)
    }
}

fn parse_pid(instance: &str) -> Option<u32> {
    instance.strip_prefix("pid_")?.split('_').next()?.parse().ok()
}

/// "pid_1234_luid_0x00000000_0x0000D1B5_phys_0_eng_0_engtype_3D"
fn parse_engine_instance(instance: &str) -> Option<(u32, String, String)> {
    let pid = parse_pid(instance)?;
    let luid_start = instance.find("luid_")? + 5;
    let luid_end = instance[luid_start..].find("_phys")? + luid_start;
    let engtype = instance.rsplit_once("engtype_")?.1;
    Some((pid, instance[luid_start..luid_end].to_string(), engtype.to_string()))
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

    let nv = NvGpu::new();
    let base = BaseCounters::new();
    let mut gpu_counters: Option<GpuCounters> = None;
    let mut networks = Networks::new_with_refreshed_list().ok();
    let mut last_net = Instant::now();
    let mut temp_reader: Option<crate::cpu_sensor::Reader> = None;
    let mut last_reader_attempt: Option<Instant> = None;

    let mut next = Instant::now() + TICK;
    loop {
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
        next += TICK;
        if next < Instant::now() {
            next = Instant::now() + TICK;
        }

        let v = *view.lock().unwrap();
        if !v.visible {
            gpu_counters = None;
            continue;
        }

        sys.refresh_cpu_usage();
        sys.refresh_memory();

        // Effective clock = nominal frequency * % performance (matches Task Manager).
        let mut clock_ghz = None;
        let mut cpu_power_w = None;
        let mut disk = None;
        if let Some(b) = &base {
            b.query.collect();
            if let (Some(perf), Some(freq)) = (
                b.cpu_perf.as_ref().and_then(Counter::value),
                b.cpu_freq.as_ref().and_then(Counter::value),
            ) {
                clock_ghz = Some((freq * perf / 100.0 / 1000.0) as f32);
            }
            cpu_power_w = b.cpu_power.as_ref().and_then(Counter::value).map(|mw| (mw / 1000.0) as f32);
            if v.expanded
                && let (Some(idle), Some(bytes)) = (
                    b.disk_idle.as_ref().and_then(Counter::value),
                    b.disk_bytes.as_ref().and_then(Counter::value),
                )
            {
                disk = Some(DiskStats { active_pct: (100.0 - idle).clamp(0.0, 100.0), bytes_per_sec: bytes });
            }
        }

        // CPU temperature from the elevated helper, if it is running.
        if temp_reader.is_none() && last_reader_attempt.is_none_or(|t| t.elapsed() >= Duration::from_secs(5)) {
            last_reader_attempt = Some(Instant::now());
            temp_reader = crate::cpu_sensor::Reader::open();
        }
        let cpu_temp_c = temp_reader.as_ref().and_then(crate::cpu_sensor::Reader::temp_c);

        // Per-process GPU counters: only when needed.
        let want_gpu_counters = (v.expanded && v.panel == Panel::Gpu) || nv.is_none();
        if want_gpu_counters && gpu_counters.is_none() {
            gpu_counters = GpuCounters::new();
        } else if !want_gpu_counters {
            gpu_counters = None;
        }
        let gpu_sample = gpu_counters.as_ref().map(GpuCounters::sample);

        let gpu = match (&nv, nv.as_ref().and_then(NvGpu::sample)) {
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
            }),
            _ => gpu_sample.as_ref().map(|(_, _, overall)| GpuStats {
                name: "GPU".into(),
                usage: *overall,
                temp_c: None,
                power_w: None,
                power_limit_w: None,
                vram_used: None,
                vram_total: None,
                fan_pct: None,
                clock_mhz: None,
            }),
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
            Some(top_processes(&sys, v.panel, ncpu, gpu_sample.as_ref()))
        } else {
            None
        };

        let stats = Stats {
            host: host.clone(),
            uptime_secs: System::uptime().unwrap_or(0),
            cpu: CpuStats {
                name: cpu_name.clone(),
                usage: sys.global_cpu_usage(),
                clock_ghz,
                temp_c: cpu_temp_c,
                power_w: cpu_power_w,
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
            disk,
            procs_for: procs.as_ref().map(|_| v.panel),
            procs,
        };
        let _ = app.emit("stats", stats);
    }
}

/// Groups processes by executable name (e.g. all chrome.exe together) and
/// returns the top entries for the open panel.
fn top_processes(
    sys: &System,
    panel: Panel,
    ncpu: f32,
    gpu: Option<&(HashMap<u32, f32>, HashMap<u32, u64>, f32)>,
) -> Vec<ProcStats> {
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
        if let Some((per_pid, vram, _)) = gpu {
            g.gpu += per_pid.get(&pid).copied().unwrap_or(0.0);
            g.vram += vram.get(&pid).copied().unwrap_or(0);
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
