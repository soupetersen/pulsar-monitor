//! Background sampler: collects system stats once per second and hands them
//! to the UI through a callback. It only gathers what the current view shows, so the compact
//! widget stays cheap and a hidden widget costs nothing.
//!
//! Everything `sysinfo` and NVML provide is read here on every platform;
//! OS-specific extras come from `crate::monitor::platform`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sysinfo::{Networks, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::monitor::gpu::NvGpu;
use crate::monitor::platform::{GpuProcs, Platform, Want};

const TICK: Duration = Duration::from_secs(1);
const FIRST_TICK: Duration = Duration::from_millis(300);

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
pub struct Stats {
    pub host: String,
    pub uptime_secs: u64,
    pub cpu: CpuStats,
    pub ram: RamStats,
    pub gpu: Option<GpuStats>,
    pub net: Option<NetStats>,
    pub disk: Option<DiskStats>,
    pub procs: Option<Vec<ProcStats>>,
    pub procs_for: Option<Panel>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CpuStats {
    pub name: String,
    pub usage: f32,
    pub clock_ghz: Option<f32>,
    pub temp_c: Option<f32>,
    pub power_w: Option<f32>,
    pub threads: usize,
    pub cores: Vec<f32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RamStats {
    pub used: u64,
    pub total: u64,
    pub available: u64,
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
pub struct NetStats {
    pub rx_per_sec: f64,
    pub tx_per_sec: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiskStats {
    pub active_pct: f64,
    pub bytes_per_sec: f64,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProcStats {
    pub name: String,
    pub count: u32,
    pub cpu: f32,
    pub mem: u64,
    pub gpu: f32,
    pub vram: u64,
}

/// Starts the sampler thread. `on_stats` runs on that thread once per second
/// while the widget is visible.
pub fn spawn(view: SharedView, on_stats: impl FnMut(Stats) + Send + 'static) {
    std::thread::Builder::new()
        .name("sampler".into())
        .spawn(move || run(view, on_stats))
        .expect("failed to start sampler thread");
}

fn run(view: SharedView, mut on_stats: impl FnMut(Stats)) {
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
                ProcessRefreshKind::nothing().with_cpu().with_memory().with_exe(UpdateKind::OnlyIfNotSet),
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
        on_stats(stats);
    }
}

/// The executable's file name. On Linux the process name is the main
/// thread's name, cut to 15 characters, which apps often rename (Electron
/// and Firefox processes show up as "MainThread", for example).
fn process_name(p: &sysinfo::Process) -> String {
    if cfg!(target_os = "linux") {
        if let Some(file) = p.exe().and_then(|e| e.file_name()) {
            return file.to_string_lossy().into_owned();
        }
    }
    p.name().to_string_lossy().into_owned()
}

/// Linux: resident memory minus pages shared with other processes (shared
/// libraries, memory shared between Chrome's processes...). Plain resident
/// memory counts those pages once per process, so multi-process apps like
/// Chrome looked twice their size. Same measure as GNOME System Monitor.
#[cfg(target_os = "linux")]
fn private_memory(pid: u32) -> Option<u64> {
    use std::sync::OnceLock;
    static PAGE: OnceLock<u64> = OnceLock::new();
    let page = *PAGE.get_or_init(|| {
        // SAFETY: sysconf has no preconditions.
        let size = unsafe { libc_sysconf_pagesize() };
        if size > 0 { size as u64 } else { 4096 }
    });
    let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    let mut fields = statm.split_whitespace().skip(1).map(|f| f.parse::<u64>().ok());
    let resident = fields.next()??;
    let shared = fields.next()??;
    Some(resident.saturating_sub(shared) * page)
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn sysconf(name: i32) -> i64;
}

#[cfg(target_os = "linux")]
unsafe fn libc_sysconf_pagesize() -> i64 {
    const SC_PAGESIZE: i32 = 30;
    unsafe { sysconf(SC_PAGESIZE) }
}

/// Windows: each process's private working set, the memory Task Manager
/// shows. sysinfo reports the whole working set, which also counts shared
/// pages (DLLs, the GPU driver's mapped memory...) once per process. One
/// system call returns it for every process.
#[cfg(all(windows, target_pointer_width = "64"))]
fn private_memory_all() -> HashMap<u32, u64> {
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQuerySystemInformation(class: i32, buf: *mut std::ffi::c_void, len: u32, ret: *mut u32) -> i32;
    }
    const SYSTEM_PROCESS_INFORMATION: i32 = 5;
    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
    // SYSTEM_PROCESS_INFORMATION on 64-bit Windows.
    const NEXT_ENTRY: usize = 0x00;
    const WORKING_SET_PRIVATE: usize = 0x08;
    const PROCESS_ID: usize = 0x50;

    let mut buf: Vec<u64> = vec![0; 64 * 1024];
    loop {
        let bytes = (buf.len() * 8) as u32;
        let mut needed = 0u32;
        // SAFETY: the buffer is writable for `bytes` bytes.
        let status = unsafe { NtQuerySystemInformation(SYSTEM_PROCESS_INFORMATION, buf.as_mut_ptr().cast(), bytes, &mut needed) };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            // Processes may start in between: leave some room.
            buf.resize((needed as usize).max(bytes as usize) / 8 * 2, 0);
            continue;
        }
        if status < 0 {
            return HashMap::new();
        }
        break;
    }

    // SAFETY: the u64 buffer is 8-byte aligned, and the entries Windows
    // wrote stay within it.
    let base = buf.as_ptr().cast::<u8>();
    let read = |at: usize, len: usize| -> u64 {
        let mut v = [0u8; 8];
        unsafe { std::ptr::copy_nonoverlapping(base.add(at), v.as_mut_ptr(), len) };
        u64::from_le_bytes(v)
    };
    let mut map = HashMap::new();
    let mut at = 0;
    loop {
        let pid = read(at + PROCESS_ID, 8) as u32;
        map.insert(pid, read(at + WORKING_SET_PRIVATE, 8));
        let next = read(at + NEXT_ENTRY, 4) as usize;
        if next == 0 || at + next + PROCESS_ID + 8 > buf.len() * 8 {
            break;
        }
        at += next;
    }
    map
}

#[cfg(not(all(windows, target_pointer_width = "64")))]
fn private_memory_all() -> HashMap<u32, u64> {
    HashMap::new()
}

#[cfg(not(target_os = "linux"))]
fn private_memory(_pid: u32) -> Option<u64> {
    None
}

/// Groups processes by executable name (e.g. all chrome.exe together) and
/// returns them all, the busiest first, for the open panel.
fn top_processes(sys: &System, panel: Panel, ncpu: f32, gpu: Option<&GpuProcs>) -> Vec<ProcStats> {
    let mut groups: HashMap<String, ProcStats> = HashMap::new();
    let private = private_memory_all();
    for (pid, p) in sys.processes() {
        let pid = pid.as_u32();
        // On Linux sysinfo also lists threads (Chrome's "ThreadPoolForeg"...),
        // each reporting the whole process memory. Count real processes only.
        if pid == 0 || p.thread_kind().is_some() {
            continue;
        }
        let name = process_name(p);
        let g = groups.entry(name.clone()).or_insert_with(|| ProcStats { name, ..Default::default() });
        g.count += 1;
        g.cpu += p.cpu_usage() / ncpu;
        g.mem += private.get(&pid).copied().or_else(|| private_memory(pid)).unwrap_or_else(|| p.memory());
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
    list
}
