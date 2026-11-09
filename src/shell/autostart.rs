//! Start at login. Uses the same entries as the Tauri version (named
//! "Pulsar"), so the choice made there carries over:
//! - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//! - Linux: `~/.config/autostart/Pulsar.desktop`
//! - macOS: `~/Library/LaunchAgents/Pulsar.plist`

const NAME: &str = "Pulsar";

/// The executable to start: the AppImage itself when running from one.
fn exe() -> Option<String> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Some(appimage.to_string_lossy().into_owned());
    }
    std::env::current_exe().ok().map(|p| p.to_string_lossy().into_owned())
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
        RegQueryValueExW, RegSetValueExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn open(access: u32) -> Option<HKEY> {
        let mut key = std::ptr::null_mut();
        let ok = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN).as_ptr(), 0, access, &mut key) } == 0;
        ok.then_some(key)
    }

    pub fn is_enabled() -> bool {
        let Some(key) = open(KEY_READ) else { return false };
        let found = unsafe {
            RegQueryValueExW(
                key,
                wide(super::NAME).as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        } == 0;
        unsafe { RegCloseKey(key) };
        found
    }

    pub fn set(enabled: bool) {
        let Some(key) = open(KEY_SET_VALUE) else { return };
        unsafe {
            if enabled {
                if let Some(exe) = super::exe() {
                    let value = wide(&format!("\"{exe}\""));
                    RegSetValueExW(
                        key,
                        wide(super::NAME).as_ptr(),
                        0,
                        REG_SZ,
                        value.as_ptr() as *const u8,
                        (value.len() * 2) as u32,
                    );
                }
            } else {
                RegDeleteValueW(key, wide(super::NAME).as_ptr());
            }
            RegCloseKey(key);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::PathBuf;

    fn entry() -> Option<PathBuf> {
        let home = PathBuf::from(std::env::var_os("HOME")?);
        Some(if cfg!(target_os = "macos") {
            home.join("Library/LaunchAgents").join(format!("{}.plist", super::NAME))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))
                .join("autostart")
                .join(format!("{}.desktop", super::NAME))
        })
    }

    pub fn is_enabled() -> bool {
        entry().is_some_and(|e| e.exists())
    }

    pub fn set(enabled: bool) {
        let Some(path) = entry() else { return };
        if !enabled {
            let _ = std::fs::remove_file(path);
            return;
        }
        let Some(exe) = super::exe() else { return };
        let content = if cfg!(target_os = "macos") {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{name}</string>
  <key>ProgramArguments</key><array><string>{exe}</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#,
                name = super::NAME
            )
        } else {
            format!(
                "[Desktop Entry]\nType=Application\nVersion=1.0\nName={name}\nComment={name} startup script\nExec=\"{exe}\"\nStartupNotify=false\nTerminal=false\n",
                name = super::NAME
            )
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, content);
    }
}

pub use imp::{is_enabled, set};
