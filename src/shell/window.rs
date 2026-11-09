//! Native window behaviour gpui doesn't cover: always on top, rounded
//! corners and dark title colours on Windows.

use gpui::{Window, WindowBackgroundAppearance};

/// The backdrop behind the card: acrylic on Windows (gpui 0.2.2 has no Mica
/// yet), vibrancy on macOS, and plain transparency on Linux, where the card
/// itself is drawn almost opaque.
pub fn background() -> WindowBackgroundAppearance {
    if cfg!(target_os = "linux") {
        WindowBackgroundAppearance::Transparent
    } else {
        WindowBackgroundAppearance::Blurred
    }
}

#[cfg(windows)]
pub fn hwnd(window: &Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as _),
        _ => None,
    }
}

#[cfg(windows)]
pub fn apply_native_style(window: &Window) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    let Some(hwnd) = hwnd(window) else { return };
    set_on_top(hwnd, true);
    let set = |attr: u32, value: i32| unsafe {
        DwmSetWindowAttribute(hwnd, attr, &value as *const i32 as *const _, 4);
    };
    set(20, 1); // DWMWA_USE_IMMERSIVE_DARK_MODE
    set(33, 2); // DWMWA_WINDOW_CORNER_PREFERENCE: round
}

#[cfg(windows)]
pub fn set_on_top(hwnd: windows_sys::Win32::Foundation::HWND, on: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };
    let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
    unsafe { SetWindowPos(hwnd, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}

#[cfg(not(windows))]
pub fn apply_native_style(_window: &Window) {}
