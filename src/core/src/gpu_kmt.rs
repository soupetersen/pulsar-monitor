//! Vendor-neutral GPU data on Windows through the D3DKMT kernel-mode thunks,
//! the same source Task Manager uses. Works for AMD, Intel and NVIDIA on
//! WDDM 2.4+ drivers; NVIDIA cards still prefer NVML, which reports more.

use std::mem::{size_of, zeroed};

use windows_sys::Wdk::Graphics::Direct3D::{
    D3DKMTCloseAdapter, D3DKMTEnumAdapters2, D3DKMTQueryAdapterInfo, D3DKMT_ADAPTERINFO, D3DKMT_ADAPTERREGISTRYINFO,
    D3DKMT_ADAPTERTYPE, D3DKMT_ADAPTER_PERFDATA, D3DKMT_CLOSEADAPTER, D3DKMT_ENUMADAPTERS2, D3DKMT_QUERYADAPTERINFO,
    D3DKMT_SEGMENTSIZEINFO, KMTQAITYPE_ADAPTERPERFDATA, KMTQAITYPE_ADAPTERREGISTRYINFO, KMTQAITYPE_ADAPTERTYPE,
    KMTQAITYPE_GETSEGMENTSIZE, KMTQUERYADAPTERINFOTYPE,
};

pub struct Adapter {
    handle: u32,
    pub name: String,
    /// LUID as it appears in PDH instance names: "0xHIGH_0xLOW".
    pub luid: String,
    pub vram_total: u64,
}

#[derive(Debug, Default)]
pub struct Perf {
    pub temp_c: Option<f32>,
    pub fan_rpm: Option<u32>,
    /// Power draw as a percentage of the adapter's maximum.
    pub power_pct: Option<f32>,
}

fn query<T>(handle: u32, kind: KMTQUERYADAPTERINFOTYPE, data: &mut T) -> bool {
    let mut q = D3DKMT_QUERYADAPTERINFO {
        hAdapter: handle,
        Type: kind,
        pPrivateDriverData: (data as *mut T).cast(),
        PrivateDriverDataSize: size_of::<T>() as u32,
    };
    unsafe { D3DKMTQueryAdapterInfo(&mut q) >= 0 }
}

impl Adapter {
    pub fn perf(&self) -> Perf {
        let mut data = D3DKMT_ADAPTER_PERFDATA::default();
        if !query(self.handle, KMTQAITYPE_ADAPTERPERFDATA, &mut data) {
            return Perf::default();
        }
        // Drivers that don't report a value leave it at 0.
        Perf {
            temp_c: (data.Temperature > 0).then(|| data.Temperature as f32 / 10.0),
            fan_rpm: (data.FanRPM > 0).then_some(data.FanRPM),
            power_pct: (data.Power > 0).then(|| data.Power as f32 / 10.0),
        }
    }
}

impl Drop for Adapter {
    fn drop(&mut self) {
        let close = D3DKMT_CLOSEADAPTER { hAdapter: self.handle };
        unsafe { D3DKMTCloseAdapter(&close) };
    }
}

/// All hardware GPUs (software and display-only adapters are skipped).
pub fn adapters() -> Vec<Adapter> {
    let mut enumerate = D3DKMT_ENUMADAPTERS2 { NumAdapters: 0, pAdapters: std::ptr::null_mut() };
    if unsafe { D3DKMTEnumAdapters2(&mut enumerate) } < 0 || enumerate.NumAdapters == 0 {
        return Vec::new();
    }
    let mut infos: Vec<D3DKMT_ADAPTERINFO> = vec![unsafe { zeroed() }; enumerate.NumAdapters as usize];
    enumerate.pAdapters = infos.as_mut_ptr();
    if unsafe { D3DKMTEnumAdapters2(&mut enumerate) } < 0 {
        return Vec::new();
    }
    infos.truncate(enumerate.NumAdapters as usize);

    let mut list = Vec::new();
    for info in infos {
        let mut kind = D3DKMT_ADAPTERTYPE::default();
        let flags = if query(info.hAdapter, KMTQAITYPE_ADAPTERTYPE, &mut kind) { unsafe { kind.Anonymous.Value } } else { 0 };
        // D3DKMT_ADAPTERTYPE bits: 0 RenderSupported, 1 DisplaySupported, 2 SoftwareDevice.
        let render = flags & 0b001 != 0;
        let software = flags & 0b100 != 0;

        let mut registry: D3DKMT_ADAPTERREGISTRYINFO = unsafe { zeroed() };
        let name = if query(info.hAdapter, KMTQAITYPE_ADAPTERREGISTRYINFO, &mut registry) {
            let len = registry.AdapterString.iter().position(|&c| c == 0).unwrap_or(registry.AdapterString.len());
            String::from_utf16_lossy(&registry.AdapterString[..len])
        } else {
            String::new()
        };

        let mut segments = D3DKMT_SEGMENTSIZEINFO::default();
        let vram_total = if query(info.hAdapter, KMTQAITYPE_GETSEGMENTSIZE, &mut segments) { segments.DedicatedVideoMemorySize } else { 0 };

        let adapter = Adapter {
            handle: info.hAdapter,
            name: name.trim().to_string(),
            luid: format!("0x{:08X}_0x{:08X}", info.AdapterLuid.HighPart as u32, info.AdapterLuid.LowPart),
            vram_total,
        };
        if render && !software && !adapter.name.is_empty() {
            list.push(adapter);
        }
        // Skipped adapters are closed by Drop.
    }
    list
}

/// The GPU to show: the one with the most dedicated memory (discrete over
/// integrated). `PULSAR_GPU` (a name substring) overrides the choice.
pub fn main_adapter() -> Option<Adapter> {
    let mut list = adapters();
    if let Ok(wanted) = std::env::var("PULSAR_GPU") {
        let wanted = wanted.to_lowercase();
        if let Some(i) = list.iter().position(|a| a.name.to_lowercase().contains(&wanted)) {
            return Some(list.swap_remove(i));
        }
    }
    list.sort_by_key(|a| std::cmp::Reverse(a.vram_total));
    list.into_iter().next()
}

pub fn is_nvidia(name: &str) -> bool {
    name.to_lowercase().contains("nvidia")
}
