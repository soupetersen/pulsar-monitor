//! Thin wrapper over the Windows Performance Data Helper (PDH) API.
//! Used for counters sysinfo/NVML don't expose: effective CPU clock,
//! disk activity and per-process GPU usage (same source as Task Manager).

use std::ptr::null_mut;
use windows_sys::Win32::System::Performance::*;

// PDH_FMT_NOCAP100 (pdh.h): allow values above 100, e.g. turbo "% Processor Performance".
const PDH_FMT_NOCAP100: PDH_FMT = 0x8000;
const FMT: PDH_FMT = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct Query {
    handle: PDH_HQUERY,
}

/// A counter handle. Only valid while the `Query` that created it is alive,
/// so both are always kept together inside the same struct.
pub struct Counter(PDH_HCOUNTER);

impl Query {
    pub fn new() -> Option<Self> {
        let mut handle = null_mut();
        let status = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut handle) };
        (status == 0).then_some(Self { handle })
    }

    pub fn add(&self, path: &str) -> Option<Counter> {
        let path = wide(path);
        let mut counter = null_mut();
        let status = unsafe { PdhAddEnglishCounterW(self.handle, path.as_ptr(), 0, &mut counter) };
        (status == 0).then_some(Counter(counter))
    }

    pub fn collect(&self) -> bool {
        unsafe { PdhCollectQueryData(self.handle) == 0 }
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.handle);
        }
    }
}

impl Counter {
    pub fn value(&self) -> Option<f64> {
        let mut value = PDH_FMT_COUNTERVALUE::default();
        let status = unsafe { PdhGetFormattedCounterValue(self.0, FMT, null_mut(), &mut value) };
        if status == 0 && value.CStatus == 0 {
            Some(unsafe { value.Anonymous.doubleValue })
        } else {
            None
        }
    }

    /// Values of a wildcard counter, as (instance name, value).
    pub fn array(&self) -> Vec<(String, f64)> {
        let mut size = 0u32;
        let mut count = 0u32;
        let status = unsafe { PdhGetFormattedCounterArrayW(self.0, FMT, &mut size, &mut count, null_mut()) };
        if status != PDH_MORE_DATA || size == 0 {
            return Vec::new();
        }
        // The buffer holds `count` items followed by the instance-name strings.
        let item_size = size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        let mut buf: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = Vec::with_capacity((size as usize).div_ceil(item_size));
        let status = unsafe { PdhGetFormattedCounterArrayW(self.0, FMT, &mut size, &mut count, buf.as_mut_ptr()) };
        if status != 0 {
            return Vec::new();
        }
        unsafe { buf.set_len(count as usize) };
        buf.iter()
            .filter(|item| item.FmtValue.CStatus == 0 && !item.szName.is_null())
            .map(|item| {
                let name = unsafe {
                    let mut len = 0;
                    while *item.szName.add(len) != 0 {
                        len += 1;
                    }
                    String::from_utf16_lossy(std::slice::from_raw_parts(item.szName, len))
                };
                (name, unsafe { item.FmtValue.Anonymous.doubleValue })
            })
            .collect()
    }
}
