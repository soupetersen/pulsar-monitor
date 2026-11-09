//! Native window behaviour gpui doesn't cover: always on top, hiding and
//! showing, rounded corners and dark title colours on Windows.

use gpui::{Pixels, Size, Window, WindowBackgroundAppearance, px};

/// The backdrop behind the card: acrylic on Windows (gpui 0.2.2 has no Mica
/// yet), vibrancy on macOS, and a blur on KDE. Other Linux desktops (GNOME)
/// don't let apps blur what's behind them: the window is just translucent.
pub fn background() -> WindowBackgroundAppearance {
    WindowBackgroundAppearance::Blurred
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

/// Whether the window can be kept above the others (Wayland doesn't let
/// apps ask for it).
pub fn can_stay_on_top() -> bool {
    #[cfg(target_os = "linux")]
    return !wayland();
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

/// Whether the window position is saved and restored natively (Windows),
/// in physical pixels, instead of through gpui's bounds.
pub const NATIVE_POSITION: bool = cfg!(windows);

/// The window's top-left corner, to save.
#[cfg(windows)]
pub fn position(window: &Window) -> Option<(f32, f32)> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let hwnd = hwnd(window)?;
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    (unsafe { GetWindowRect(hwnd, &mut rect) } != 0).then_some((rect.left as f32, rect.top as f32))
}

#[cfg(not(windows))]
pub fn position(window: &Window) -> Option<(f32, f32)> {
    let origin = window.bounds().origin;
    Some((f32::from(origin.x), f32::from(origin.y)))
}

/// Moves the window to a saved position, unless no monitor is there
/// anymore (it then stays centered).
#[cfg(windows)]
pub fn set_position(window: &Window, (x, y): (f32, f32)) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos};
    let Some(hwnd) = hwnd(window) else { return };
    let (x, y) = (x.round() as i32, y.round() as i32);
    // A point a bit inside the window's corner must be on a screen.
    if unsafe { MonitorFromPoint(POINT { x: x + 24, y: y + 24 }, MONITOR_DEFAULTTONULL) }.is_null() {
        return;
    }
    let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
    unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), x, y, 0, 0, flags) };
}

#[cfg(not(windows))]
pub fn set_position(_window: &Window, _position: (f32, f32)) {}

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
pub fn apply_native_style(window: &Window, on_top: bool, _dark: bool) {
    // gpui makes pop-up windows float above everything already.
    if !on_top {
        set_on_top(window, false);
    }
}

#[cfg(not(windows))]
pub fn set_dark(_window: &Window, _dark: bool) {}

#[cfg(target_os = "macos")]
mod mac {
    use gpui::Window;
    use objc::runtime::{Object, YES};
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn ns_window(window: &Window) -> Option<*mut Object> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::AppKit(h) = handle.as_raw() else { return None };
        let view = h.ns_view.as_ptr() as *mut Object;
        let ns_window: *mut Object = unsafe { msg_send![view, window] };
        (!ns_window.is_null()).then_some(ns_window)
    }

    /// No Dock icon or app menu: Pulsar lives in the menu bar. gpui sets the
    /// regular policy at launch, so this runs after it.
    pub fn hide_from_dock() {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            // NSApplicationActivationPolicyAccessory
            let _: i8 = msg_send![app, setActivationPolicy: 1isize];
        }
    }

    pub fn set_level(window: &Window, level: isize) {
        if let Some(w) = ns_window(window) {
            unsafe {
                let _: () = msg_send![w, setLevel: level];
            }
        }
    }

    pub fn order_out(window: &Window) {
        if let Some(w) = ns_window(window) {
            unsafe {
                let _: () = msg_send![w, orderOut: std::ptr::null_mut::<Object>()];
            }
        }
    }

    pub fn order_front(window: &Window) {
        if let Some(w) = ns_window(window) {
            unsafe {
                let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                let _: () = msg_send![app, activateIgnoringOtherApps: YES];
                let _: () = msg_send![w, makeKeyAndOrderFront: std::ptr::null_mut::<Object>()];
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use mac::hide_from_dock;

/// Always on top: the pop-up level gpui gives the window, or the normal one.
#[cfg(target_os = "macos")]
pub fn set_on_top(window: &Window, on: bool) {
    const NS_NORMAL_WINDOW_LEVEL: isize = 0;
    const NS_POP_UP_WINDOW_LEVEL: isize = 101;
    mac::set_level(window, if on { NS_POP_UP_WINDOW_LEVEL } else { NS_NORMAL_WINDOW_LEVEL });
}

#[cfg(target_os = "macos")]
pub fn hide(window: &mut Window) {
    mac::order_out(window);
}

#[cfg(target_os = "macos")]
pub fn show(window: &mut Window) {
    mac::order_front(window);
}

/// Always on top through the window manager (_NET_WM_STATE_ABOVE). X11
/// only: Wayland doesn't let apps ask for it.
#[cfg(target_os = "linux")]
pub fn set_on_top(window: &Window, on: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if wayland() {
        return;
    }
    let Ok(handle) = HasWindowHandle::window_handle(window) else { return };
    let id = match handle.as_raw() {
        RawWindowHandle::Xcb(h) => h.window.get(),
        RawWindowHandle::Xlib(h) => h.window as u32,
        _ => return,
    };
    let _ = x11_set_above(id, on);
}

#[cfg(target_os = "linux")]
fn x11_set_above(window: u32, on: bool) -> Option<()> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ClientMessageEvent, ConnectionExt, EventMask};
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let atom = |name: &str| Some(conn.intern_atom(false, name.as_bytes()).ok()?.reply().ok()?.atom);
    let (state, above) = (atom("_NET_WM_STATE")?, atom("_NET_WM_STATE_ABOVE")?);
    // _NET_WM_STATE_REMOVE = 0, _NET_WM_STATE_ADD = 1; source: application.
    let event = ClientMessageEvent::new(32, window, state, [on as u32, above, 0, 1, 0]);
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    conn.send_event(false, root, mask, event).ok()?;
    conn.flush().ok()
}

#[cfg(target_os = "linux")]
pub fn hide(window: &mut Window) {
    window.minimize_window();
}

#[cfg(target_os = "linux")]
pub fn show(window: &mut Window) {
    window.activate_window();
}
