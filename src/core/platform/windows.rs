//! Windows: PDH performance counters (effective clock, CPU power, disk, GPU
//! usage per adapter and process), D3DKMT for non-NVIDIA GPUs and the PawnIO
//! helper for the CPU temperature.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::{Extras, GpuProcs, Want};
use crate::monitor::cpu_sensor;
use crate::monitor::gpu_kmt::{self, Adapter};
use crate::monitor::pdh::{Counter, Query};
use crate::monitor::sampler::{DiskStats, GpuStats};

pub struct Platform {
    base: Option<BaseCounters>,
    gpu_counters: Option<GpuCounters>,
    /// Main GPU when it isn't NVIDIA (or `PULSAR_GPU` picks one).
    adapter: Option<Adapter>,
    prefer_kmt: bool,
    temp_reader: Option<cpu_sensor::Reader>,
    last_reader_attempt: Option<Instant>,
}

impl Platform {
    pub fn new() -> Self {
        let adapter = gpu_kmt::main_adapter();
        let prefer_kmt =
            std::env::var_os("PULSAR_GPU").is_some() || adapter.as_ref().is_some_and(|a| !gpu_kmt::is_nvidia(&a.name));
        Self {
            base: BaseCounters::new(),
            gpu_counters: None,
            adapter,
            prefer_kmt,
            temp_reader: None,
            last_reader_attempt: None,
        }
    }

    /// NVML only when the main GPU is NVIDIA (it reports more than D3DKMT).
    pub fn wants_nvml(&self) -> bool {
        !self.prefer_kmt
    }

    pub fn sample(&mut self, want: Want) -> Extras {
        let mut extras = Extras::default();

        if let Some(b) = &self.base {
            b.query.collect();
            // Effective clock = nominal frequency * % performance (matches Task Manager).
            if let (Some(perf), Some(freq)) =
                (b.cpu_perf.as_ref().and_then(Counter::value), b.cpu_freq.as_ref().and_then(Counter::value))
            {
                extras.clock_ghz = Some((freq * perf / 100.0 / 1000.0) as f32);
            }
            extras.cpu_power_w = b.cpu_power.as_ref().and_then(Counter::value).map(|mw| (mw / 1000.0) as f32);
            if want.expanded
                && let (Some(idle), Some(bytes)) =
                    (b.disk_idle.as_ref().and_then(Counter::value), b.disk_bytes.as_ref().and_then(Counter::value))
            {
                extras.disk = Some(DiskStats { active_pct: (100.0 - idle).clamp(0.0, 100.0), bytes_per_sec: bytes });
            }
        }

        // CPU temperature from the elevated helper, if it is running.
        if self.temp_reader.is_none()
            && self.last_reader_attempt.is_none_or(|t| t.elapsed() >= Duration::from_secs(5))
        {
            self.last_reader_attempt = Some(Instant::now());
            self.temp_reader = cpu_sensor::Reader::open();
        }
        extras.cpu_temp_c = self.temp_reader.as_ref().and_then(cpu_sensor::Reader::temp_c);

        // GPU counters: for the process list, or for the adapter when not using NVML.
        let adapter = if want.nvml { None } else { self.adapter.as_ref() };
        let want_counters = want.gpu_panel || adapter.is_some();
        if want_counters && self.gpu_counters.is_none() {
            self.gpu_counters = GpuCounters::new();
        } else if !want_counters {
            self.gpu_counters = None;
        }
        let sample = self.gpu_counters.as_ref().map(GpuCounters::sample);

        if let Some(a) = adapter {
            let perf = a.perf();
            extras.gpu = Some(GpuStats {
                name: a.name.trim_start_matches("NVIDIA ").trim_start_matches("GeForce ").to_string(),
                usage: sample.as_ref().and_then(|s| s.usage_per_luid.get(&a.luid)).copied().unwrap_or(0.0),
                temp_c: perf.temp_c.map(|t| t.round() as u32),
                power_w: None,
                power_limit_w: None,
                power_pct: perf.power_pct,
                vram_used: sample.as_ref().and_then(|s| s.vram_per_luid.get(&a.luid)).copied(),
                vram_total: (a.vram_total > 0).then_some(a.vram_total),
                fan_pct: None,
                fan_rpm: perf.fan_rpm,
                clock_mhz: None,
            });
        }
        if want.gpu_panel {
            extras.gpu_procs = sample.map(|s| s.procs);
        }
        extras
    }

    pub fn on_hidden(&mut self) {
        self.gpu_counters = None;
    }
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

/// GPU counters from PDH. Enumerating GPU engines is relatively expensive, so
/// this only exists while the GPU panel is open, or when the GPU isn't read
/// through NVML (then it provides the usage and VRAM of the adapter).
struct GpuCounters {
    query: Query,
    engine: Counter,
    memory: Option<Counter>,
    adapter_memory: Option<Counter>,
}

struct GpuSample {
    procs: GpuProcs,
    /// Overall GPU % per adapter LUID ("0xHIGH_0xLOW").
    usage_per_luid: HashMap<String, f32>,
    /// Dedicated VRAM in use per adapter LUID.
    vram_per_luid: HashMap<String, u64>,
}

impl GpuCounters {
    fn new() -> Option<Self> {
        let query = Query::new()?;
        let engine = query.add(r"\GPU Engine(*)\Utilization Percentage")?;
        let memory = query.add(r"\GPU Process Memory(*)\Dedicated Usage");
        let adapter_memory = query.add(r"\GPU Adapter Memory(*)\Dedicated Usage");
        query.collect();
        Some(Self { query, engine, memory, adapter_memory })
    }

    fn sample(&self) -> GpuSample {
        self.query.collect();

        // Sum engine instances per (pid, adapter, engine type), then take the
        // busiest engine type per pid, like Task Manager's "GPU" column.
        let mut per_engine: HashMap<(u32, String, String), f64> = HashMap::new();
        for (name, value) in self.engine.array() {
            if let Some((pid, luid, engtype)) = parse_engine_instance(&name) {
                *per_engine.entry((pid, luid, engtype)).or_default() += value;
            }
        }
        let mut usage: HashMap<u32, f32> = HashMap::new();
        let mut per_type: HashMap<(String, String), f64> = HashMap::new();
        for ((pid, luid, engtype), value) in per_engine {
            let e = usage.entry(pid).or_default();
            *e = e.max(value as f32);
            *per_type.entry((luid, engtype)).or_default() += value;
        }
        // Adapter usage = its busiest engine type, like Task Manager.
        let mut usage_per_luid: HashMap<String, f32> = HashMap::new();
        for ((luid, _), value) in per_type {
            let e = usage_per_luid.entry(luid).or_default();
            *e = e.max(value.min(100.0) as f32);
        }

        let mut vram: HashMap<u32, u64> = HashMap::new();
        if let Some(mem) = &self.memory {
            for (name, value) in mem.array() {
                if let Some(pid) = parse_pid(&name) {
                    *vram.entry(pid).or_default() += value as u64;
                }
            }
        }

        // "luid_0x00000000_0x0000D1B5_phys_0"
        let mut vram_per_luid: HashMap<String, u64> = HashMap::new();
        if let Some(mem) = &self.adapter_memory {
            for (name, value) in mem.array() {
                if let Some(luid) = name.strip_prefix("luid_").and_then(|s| s.split("_phys").next()) {
                    *vram_per_luid.entry(luid.to_string()).or_default() += value as u64;
                }
            }
        }
        GpuSample { procs: GpuProcs { usage, vram }, usage_per_luid, vram_per_luid }
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
