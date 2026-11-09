//! Native window behaviour gpui doesn't cover: always on top, hiding and
//! showing, rounded corners and dark title colours on Windows.

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

#[cfg(target_os = "linux")]
fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Whether the window can be hidden and shown again in place.
pub fn can_hide() -> bool {
    #[cfg(target_os = "linux")]
    return false;
    #[cfg(not(target_os = "linux"))]
    true
}

/// Whether the window's position can be read (and later restored).
pub fn position_is_known() -> bool {
    #[cfg(target_os = "linux")]
    return !wayland();
    #[cfg(not(target_os = "linux"))]
    true
}

#[cfg(windows)]
fn hwnd(window: &Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as _),
        _ => None,
    }
}

#[cfg(windows)]
pub fn apply_native_style(window: &Window, on_top: bool) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    let Some(hwnd) = hwnd(window) else { return };
    set_on_top(window, on_top);
    let set = |attr: u32, value: i32| unsafe {
        DwmSetWindowAttribute(hwnd, attr, &value as *const i32 as *const _, 4);
    };
    set(20, 1); // DWMWA_USE_IMMERSIVE_DARK_MODE
    set(33, 2); // DWMWA_WINDOW_CORNER_PREFERENCE: round
}

#[cfg(windows)]
pub fn set_on_top(window: &Window, on: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };
    let Some(hwnd) = hwnd(window) else { return };
    let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
    unsafe { SetWindowPos(hwnd, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}

#[cfg(windows)]
pub fn hide(window: &mut Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
    if let Some(hwnd) = hwnd(window) {
        unsafe { ShowWindow(hwnd, SW_HIDE) };
    }
}

#[cfg(windows)]
pub fn show(window: &mut Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_SHOW, SetForegroundWindow, ShowWindow};
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
pub fn apply_native_style(_window: &Window, _on_top: bool) {}

// TODO(phase 2): always on top on Linux (_NET_WM_STATE_ABOVE on X11; not
// available to Wayland clients) and macOS (NSWindow level).
#[cfg(not(windows))]
pub fn set_on_top(_window: &Window, _on: bool) {}

#[cfg(not(windows))]
pub fn hide(window: &mut Window) {
    window.minimize_window();
}

#[cfg(not(windows))]
pub fn show(window: &mut Window) {
    window.activate_window();
}
