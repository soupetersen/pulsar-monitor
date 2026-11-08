//! CPU temperature for AMD Ryzen and Intel Core through the PawnIO driver.
//!
//! PawnIO only accepts administrators, so the reading happens in a tiny
//! elevated helper: this same executable started with `--cpu-sensor` by a
//! scheduled task at logon. The helper publishes the value through shared
//! memory and the widget itself stays unelevated.
//!
//! The scheduled task points at a copy of the executable under Program Files
//! (writable only by administrators), never at the per-user install folder,
//! so other user processes can't swap the binary that runs elevated.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_ALREADY_EXISTS, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED,
    WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_ALL_ACCESS, FILE_MAP_READ,
    MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
};
use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetExitCodeProcess, ReleaseMutex, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

pub const HELPER_ARG: &str = "--cpu-sensor";
pub const ENABLE_ARG: &str = "--enable-cpu-sensor";
pub const DISABLE_ARG: &str = "--disable-cpu-sensor";

const MAPPING_NAME: &str = r"Local\PulsarCpuSensor";
const INSTANCE_MUTEX: &str = r"Local\PulsarCpuSensorInstance";
/// Shared with other hardware monitors (LibreHardwareMonitor, FanControl, ...)
/// so SMN accesses from different programs don't interleave.
const PCI_MUTEX: &str = r"Global\Access_PCI";
const TASK_NAME: &str = "Pulsar CPU Sensor";
/// Names used before the app was renamed to Pulsar (cleaned up on install).
const LEGACY_TASK_NAME: &str = "Widget Monitor CPU Sensor";
const LEGACY_HELPER_EXE: &str = "widget-cpu-sensor.exe";
const LEGACY_DIR: &str = "Widget Monitor";
const APP_DIR: &str = "Pulsar";
const HELPER_EXE: &str = "pulsar-cpu-sensor.exe";
const MAGIC: u32 = 0x5443_5350; // "PSCT"
const STALE_AFTER_MS: u64 = 5_000;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Signed PawnIO module for AMD family 17h–1Ah (Zen 1–5), LGPL-2.1.
const AMD_MODULE: &[u8] = include_bytes!("../resources/pawnio/AMDFamily17.bin");
/// SMN register THM_TCON_CUR_TMP: current Tctl.
const SMN_THM_TCON_CUR_TMP: u64 = 0x0005_9800;

/// Signed PawnIO module for Intel MSRs (read-only allowlist includes the thermal MSRs), LGPL-2.1.
const INTEL_MODULE: &[u8] = include_bytes!("../resources/pawnio/IntelMSR.bin");
/// MSR_TEMPERATURE_TARGET: bits 23:16 hold TjMax.
const MSR_TEMPERATURE_TARGET: u64 = 0x1a2;
/// IA32_PACKAGE_THERM_STATUS: bits 22:16 hold how far below TjMax the package is.
const MSR_PACKAGE_THERM_STATUS: u64 = 0x1b1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vendor {
    Amd,
    Intel,
}

impl Vendor {
    fn module(self) -> (&'static str, &'static [u8]) {
        match self {
            Vendor::Amd => ("AMDFamily17", AMD_MODULE),
            Vendor::Intel => ("IntelMSR", INTEL_MODULE),
        }
    }
}

#[repr(C)]
struct Shared {
    magic: AtomicU32,
    temp_milli_c: AtomicI32,
    updated_ms: AtomicU64,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn program_files() -> PathBuf {
    std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
}

fn pawnio_dll() -> PathBuf {
    program_files().join(r"PawnIO\PawnIOLib.dll")
}

/// CPU vendor, if it's one the sensor supports.
pub fn vendor() -> Option<Vendor> {
    let id = std::arch::x86_64::__cpuid(0);
    let vendor: Vec<u8> = [id.ebx, id.edx, id.ecx].iter().flat_map(|r| r.to_le_bytes()).collect();
    match vendor.as_slice() {
        b"AuthenticAMD" => Some(Vendor::Amd),
        b"GenuineIntel" => Some(Vendor::Intel),
        _ => None,
    }
}

pub fn pawnio_installed() -> bool {
    pawnio_dll().exists()
}

/// Appends a line to %TEMP%\pulsar-cpu-sensor.log (same file for the
/// widget, the elevated installer and the helper, since they run as the same user).
pub fn log(msg: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("pulsar-cpu-sensor.log");
    // Keep the log small.
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{}] [pid {}] {msg}", now_ms() / 1000, std::process::id());
    }
}

fn schtasks_logged(args: &[&str]) -> bool {
    match Command::new("schtasks.exe").args(args).creation_flags(CREATE_NO_WINDOW).output() {
        Ok(o) => {
            if !o.status.success() {
                log(&format!(
                    "schtasks {:?} failed ({}): {} {}",
                    args,
                    o.status,
                    String::from_utf8_lossy(&o.stdout).trim(),
                    String::from_utf8_lossy(&o.stderr).trim()
                ));
            }
            o.status.success()
        }
        Err(e) => {
            log(&format!("schtasks {args:?} could not start: {e}"));
            false
        }
    }
}

fn schtasks(args: &[&str]) -> bool {
    Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Whether the helper task is registered.
pub fn enabled() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME])
}

// ---------------------------------------------------------------------------
// Widget side: read the value published by the helper.

pub struct Reader {
    map: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}

impl Reader {
    pub fn open() -> Option<Self> {
        let name = wide(MAPPING_NAME);
        let map = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, name.as_ptr()) };
        if map.is_null() {
            return None;
        }
        let view = unsafe { MapViewOfFile(map, FILE_MAP_READ, 0, 0, size_of::<Shared>()) };
        if view.Value.is_null() {
            unsafe { CloseHandle(map) };
            return None;
        }
        Some(Self { map, view })
    }

    /// Latest Tctl in °C, if the helper updated it recently.
    pub fn temp_c(&self) -> Option<f32> {
        let shared = unsafe { &*(self.view.Value as *const Shared) };
        if shared.magic.load(Ordering::Acquire) != MAGIC {
            return None;
        }
        let age = now_ms().saturating_sub(shared.updated_ms.load(Ordering::Acquire));
        (age < STALE_AFTER_MS).then(|| shared.temp_milli_c.load(Ordering::Acquire) as f32 / 1000.0)
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.view);
            CloseHandle(self.map);
        }
    }
}

// ---------------------------------------------------------------------------
// Helper side (elevated): read Tctl through PawnIO and publish it.

type OpenFn = unsafe extern "system" fn(*mut HANDLE) -> i32;
type LoadFn = unsafe extern "system" fn(HANDLE, *const u8, usize) -> i32;
type ExecFn = unsafe extern "system" fn(HANDLE, *const u8, *const u64, usize, *mut u64, usize, *mut usize) -> i32;
type CloseFn = unsafe extern "system" fn(HANDLE) -> i32;

struct PawnIo {
    _lib: libloading::Library,
    handle: HANDLE,
    exec: ExecFn,
    close: CloseFn,
}

impl PawnIo {
    fn open(vendor: Vendor) -> Option<Self> {
        let (module_name, module) = vendor.module();
        unsafe {
            let lib = match libloading::Library::new(pawnio_dll()) {
                Ok(lib) => lib,
                Err(e) => {
                    log(&format!("could not load {}: {e}", pawnio_dll().display()));
                    return None;
                }
            };
            let open: OpenFn = *lib.get::<OpenFn>(b"pawnio_open\0").ok()?;
            let load: LoadFn = *lib.get::<LoadFn>(b"pawnio_load\0").ok()?;
            let exec: ExecFn = *lib.get::<ExecFn>(b"pawnio_execute\0").ok()?;
            let close: CloseFn = *lib.get::<CloseFn>(b"pawnio_close\0").ok()?;
            let mut handle: HANDLE = null_mut();
            let hr = open(&mut handle);
            if hr < 0 {
                log(&format!("pawnio_open failed: HRESULT 0x{:08X}", hr as u32));
                return None;
            }
            let hr = load(handle, module.as_ptr(), module.len());
            if hr < 0 {
                log(&format!("pawnio_load({module_name}) failed: HRESULT 0x{:08X}", hr as u32));
                close(handle);
                return None;
            }
            Some(Self { _lib: lib, handle, exec, close })
        }
    }

    /// Runs a one-in/one-out ioctl of the loaded module.
    fn call(&self, name: &std::ffi::CStr, arg: u64) -> Option<u64> {
        let mut out = 0u64;
        let mut written = 0usize;
        let hr = unsafe { (self.exec)(self.handle, name.as_ptr().cast(), &arg, 1, &mut out, 1, &mut written) };
        if hr < 0 || written != 1 {
            log(&format!("{}(0x{arg:X}) failed: HRESULT 0x{:08X}, written {written}", name.to_string_lossy(), hr as u32));
            return None;
        }
        Some(out)
    }
}

impl Drop for PawnIo {
    fn drop(&mut self) {
        unsafe { (self.close)(self.handle) };
    }
}

/// AMD: Tctl from the SMN thermal register.
fn read_amd(pawn: &PawnIo, pci_mutex: HANDLE) -> Option<f32> {
    let wait = unsafe { WaitForSingleObject(pci_mutex, 200) };
    if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
        return None;
    }
    let raw = pawn.call(c"ioctl_read_smn", SMN_THM_TCON_CUR_TMP);
    unsafe { ReleaseMutex(pci_mutex) };
    let raw = raw? as u32;
    // Bits 31:21 hold the temperature in 1/8 °C; bit 19 selects the -49 °C range.
    let mut temp = ((raw >> 21) & 0x7ff) as f32 / 8.0;
    if raw & 0x8_0000 != 0 {
        temp -= 49.0;
    }
    Some(temp)
}

/// Intel: TjMax (MSR 0x1A2) once at startup; 100 °C if the CPU doesn't report it.
fn read_intel_tjmax(pawn: &PawnIo) -> f32 {
    match pawn.call(c"ioctl_read_msr", MSR_TEMPERATURE_TARGET).map(|v| (v >> 16) & 0xff) {
        Some(tjmax) if tjmax > 0 => tjmax as f32,
        _ => 100.0,
    }
}

/// Intel: package temperature = TjMax - digital readout (MSR 0x1B1, bits 22:16).
fn read_intel(pawn: &PawnIo, tjmax: f32) -> Option<f32> {
    let raw = pawn.call(c"ioctl_read_msr", MSR_PACKAGE_THERM_STATUS)?;
    Some(tjmax - ((raw >> 16) & 0x7f) as f32)
}

/// Entry point for `--cpu-sensor`. Runs until killed.
pub fn run_helper() -> i32 {
    unsafe {
        let instance = CreateMutexW(null(), 1, wide(INSTANCE_MUTEX).as_ptr());
        if instance.is_null() || GetLastError() == ERROR_ALREADY_EXISTS {
            log("helper: another instance is already running");
            return 0;
        }
        let Some(cpu) = vendor() else {
            log("helper: unsupported CPU vendor");
            return 1;
        };
        log(&format!("helper: starting ({cpu:?})"));

        let Some(pawn) = PawnIo::open(cpu) else { return 1 };
        let tjmax = if cpu == Vendor::Intel { read_intel_tjmax(&pawn) } else { 0.0 };

        // Readable by interactive users (the unelevated widget), full access for SYSTEM/admins.
        let sddl = wide("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GR;;;IU)");
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, null_mut()) == 0 {
            log(&format!("helper: security descriptor failed: error {}", GetLastError()));
            return 2;
        }
        let sa = SECURITY_ATTRIBUTES { nLength: size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd, bInheritHandle: 0 };
        let name = wide(MAPPING_NAME);
        let map = CreateFileMappingW(INVALID_HANDLE_VALUE, &sa, PAGE_READWRITE, 0, size_of::<Shared>() as u32, name.as_ptr());
        LocalFree(sd);
        if map.is_null() {
            log(&format!("helper: CreateFileMapping failed: error {}", GetLastError()));
            return 3;
        }
        let view = MapViewOfFile(map, FILE_MAP_ALL_ACCESS, 0, 0, size_of::<Shared>());
        if view.Value.is_null() {
            log(&format!("helper: MapViewOfFile failed: error {}", GetLastError()));
            return 4;
        }
        let shared = &*(view.Value as *const Shared);
        let pci_mutex = CreateMutexW(null(), 0, wide(PCI_MUTEX).as_ptr());

        let mut first = true;
        loop {
            let reading = match cpu {
                Vendor::Amd => read_amd(&pawn, pci_mutex),
                Vendor::Intel => read_intel(&pawn, tjmax),
            };
            if first {
                log(&format!("helper: first reading: {reading:?} C"));
                first = false;
            }
            if let Some(temp) = reading {
                shared.temp_milli_c.store((temp * 1000.0) as i32, Ordering::Release);
                shared.updated_ms.store(now_ms(), Ordering::Release);
                shared.magic.store(MAGIC, Ordering::Release);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

// ---------------------------------------------------------------------------
// Enabling / disabling (elevated one-off actions).

/// Re-runs this executable elevated (UAC prompt) to enable or disable the
/// helper, waits for it, and returns whether the helper is enabled afterwards.
pub fn request_toggle(enable: bool) -> bool {
    let Ok(exe) = std::env::current_exe() else { return enabled() };
    let user = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default()
    );
    let params = if enable { format!("{ENABLE_ARG} \"{user}\"") } else { DISABLE_ARG.to_string() };
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params = wide(&params);
    log(&format!("request_toggle(enable={enable}): asking for elevation"));
    unsafe {
        // ShellExecuteEx requires COM on the calling thread.
        CoInitializeEx(null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32);
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) != 0 && !info.hProcess.is_null() {
            WaitForSingleObject(info.hProcess, 60_000);
            let mut code = 0u32;
            GetExitCodeProcess(info.hProcess, &mut code);
            CloseHandle(info.hProcess);
            log(&format!("request_toggle: elevated step exited with code {code}"));
        } else {
            // 1223 = the user cancelled the UAC prompt.
            log(&format!("request_toggle: ShellExecuteEx failed: error {}", GetLastError()));
        }
        CoUninitialize();
    }
    let now = enabled();
    log(&format!("request_toggle: enabled is now {now}"));
    now
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `--enable-cpu-sensor "DOMAIN\user"` (elevated): copies the executable to
/// Program Files and registers + starts the logon task.
pub fn install(user: &str) -> i32 {
    log(&format!("install: user {user:?}"));
    let Ok(exe) = std::env::current_exe() else { return 1 };
    let dir = program_files().join(APP_DIR);
    let helper = dir.join(HELPER_EXE);

    // Stop a previous helper so its binary can be replaced.
    schtasks(&["/End", "/TN", TASK_NAME]);
    let _ = Command::new("taskkill.exe").args(["/F", "/IM", HELPER_EXE]).creation_flags(CREATE_NO_WINDOW).output();
    remove_legacy();
    std::thread::sleep(Duration::from_millis(500));

    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::copy(&exe, &helper)) {
        log(&format!("install: copying to {} failed: {e}", helper.display()));
        return 2;
    }

    let user = xml_escape(user);
    let command = xml_escape(&helper.to_string_lossy());
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Reads the CPU temperature for Pulsar.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{command}"</Command>
      <Arguments>{HELPER_ARG}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    );
    // schtasks expects the XML file as UTF-16 LE with a BOM.
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(xml.encode_utf16().flat_map(|u| u.to_le_bytes()));
    let xml_path = std::env::temp_dir().join("pulsar-cpu-sensor.xml");
    if let Err(e) = std::fs::write(&xml_path, bytes) {
        log(&format!("install: writing task XML failed: {e}"));
        return 3;
    }
    let xml_arg = xml_path.to_string_lossy().into_owned();
    let created = schtasks_logged(&["/Create", "/TN", TASK_NAME, "/XML", &xml_arg, "/F"]);
    let _ = std::fs::remove_file(&xml_path);
    if !created {
        return 4;
    }
    let started = schtasks_logged(&["/Run", "/TN", TASK_NAME]);
    log(&format!("install: task created, started={started}"));
    0
}

/// `--disable-cpu-sensor` (elevated): removes the task and the helper copy.
pub fn uninstall() -> i32 {
    remove_task(TASK_NAME, HELPER_EXE, APP_DIR);
    remove_legacy();
    0
}

/// Removes the task and helper installed under the app's previous name.
fn remove_legacy() {
    remove_task(LEGACY_TASK_NAME, LEGACY_HELPER_EXE, LEGACY_DIR);
}

fn remove_task(task: &str, exe: &str, dir: &str) {
    schtasks(&["/End", "/TN", task]);
    schtasks(&["/Delete", "/TN", task, "/F"]);
    let _ = Command::new("taskkill.exe").args(["/F", "/IM", exe]).creation_flags(CREATE_NO_WINDOW).output();
    std::thread::sleep(Duration::from_millis(300));
    let dir = program_files().join(dir);
    let _ = std::fs::remove_file(dir.join(exe));
    let _ = std::fs::remove_dir(&dir);
}
