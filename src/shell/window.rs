//! Native window behaviour gpui doesn't cover: always on top, hiding and
//! showing, rounded corners and dark title colours on Windows.

use gpui::{Pixels, Size, Window, WindowBackgroundAppearance, px};

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
fn dwm_set(window: &Window, attr: u32, value: i32) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    let Some(hwnd) = hwnd(window) else { return };
    unsafe { DwmSetWindowAttribute(hwnd, attr, &value as *const i32 as *const _, 4) };
}

#[cfg(windows)]
pub fn apply_native_style(window: &Window, on_top: bool, dark: bool) {
    set_on_top(window, on_top);
    set_dark(window, dark);
    dwm_set(window, 33, 2); // DWMWA_WINDOW_CORNER_PREFERENCE: round
}

/// Dark or light window border.
#[cfg(windows)]
pub fn set_dark(window: &Window, dark: bool) {
    dwm_set(window, 20, dark as i32); // DWMWA_USE_IMMERSIVE_DARK_MODE
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

/// Gap kept between the window and the edges of the screen.
const MARGIN: f32 = 12.0;

/// Resizes the window to `size`, never taller than the screen's work area.
/// When it grows past the bottom of the screen, it slides up (Windows).
#[cfg(windows)]
pub fn fit(window: &mut Window, size: Size<Pixels>, _cx: &gpui::App) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    };
    let Some(hwnd) = hwnd(window) else { return };
    let scale = window.scale_factor();
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    let work = unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        (GetMonitorInfoW(monitor, &mut info) != 0).then_some(info.rcWork)
    };
    let mut size = size;
    if let Some(work) = work {
        let max = (work.bottom - work.top) as f32 / scale - 2.0 * MARGIN;
        size.height = size.height.min(px(max));
    }
    window.resize(size);

    let Some(work) = work else { return };
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return;
    }
    let height = (f32::from(size.height) * scale).ceil() as i32;
    let margin = (MARGIN * scale).round() as i32;
    if rect.top + height > work.bottom - margin {
        let y = (work.bottom - margin - height).max(work.top + margin);
        let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
        unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), rect.left, y, 0, 0, flags) };
    }
}

/// Resizes the window to `size`, never taller than the screen. gpui can't
/// move windows here, so one growing past the bottom isn't moved.
#[cfg(not(windows))]
pub fn fit(window: &mut Window, size: Size<Pixels>, cx: &gpui::App) {
    let mut size = size;
    if let Some(display) = window.display(cx) {
        size.height = size.height.min(display.bounds().size.height - px(2.0 * MARGIN));
    }
    window.resize(size);
}

#[cfg(not(windows))]
pub fn apply_native_style(_window: &Window, _on_top: bool, _dark: bool) {}

#[cfg(not(windows))]
pub fn set_dark(_window: &Window, _dark: bool) {}

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
