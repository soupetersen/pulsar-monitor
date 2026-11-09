//! Phase 0 spike (issue #6): the compact view drawn with gpui, fed by real
//! data once per second, plus a tray icon. `SPIKE_BG=transparent|mica|blur|opaque`
//! picks the window backdrop to compare.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    App, Application, Bounds, Context, FontWeight, MouseButton, PathBuilder, Render, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, canvas, div, point, prelude::*, px,
    rgb, rgba, size,
};
use sysinfo::System;

#[derive(Clone, Default)]
struct Gpu {
    usage: u32,
    temp: Option<u32>,
    vram_used: u64,
    vram_total: u64,
}

#[derive(Clone, Default)]
struct Sample {
    cpu: f32,
    ram_used: u64,
    ram_total: u64,
    gpu: Option<Gpu>,
}

type Shared = Arc<Mutex<Option<Sample>>>;

fn spawn_sampler(shared: Shared) {
    std::thread::spawn(move || {
        let mut sys = System::new();
        let nvml = nvml_wrapper::Nvml::init().ok();
        loop {
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            let gpu = nvml.as_ref().and_then(|n| n.device_by_index(0).ok()).map(|d| {
                let mem = d.memory_info().ok();
                Gpu {
                    usage: d.utilization_rates().map(|u| u.gpu).unwrap_or(0),
                    temp: d.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).ok(),
                    vram_used: mem.as_ref().map(|m| m.used).unwrap_or(0),
                    vram_total: mem.as_ref().map(|m| m.total).unwrap_or(0),
                }
            });
            *shared.lock().unwrap() = Some(Sample {
                cpu: sys.global_cpu_usage(),
                ram_used: sys.used_memory(),
                ram_total: sys.total_memory(),
                gpu,
            });
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

// Dark theme tokens from app.css.
const TEXT: u32 = 0xf3f4f6;
const MUTED: u32 = 0xb4b9c2;
const ACCENT: u32 = 0x4cc2ff;
const CARD: u32 = 0x1c1d228c; // rgba(28,29,34,0.55)
const TRACK: u32 = 0xffffff1f; // 0.12
const HOVER: u32 = 0xffffff17; // 0.09

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

struct Compact {
    sample: Option<Sample>,
    host: String,
}

impl Compact {
    fn new(shared: Shared, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(1000)).await;
                let sample = shared.lock().unwrap().clone();
                if this
                    .update(cx, |view, cx| {
                        view.sample = sample;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self { sample: None, host: System::host_name().unwrap_or_default() }
    }
}

fn ring(index: usize, value: f32, label: String) -> impl IntoElement {
    let value = value.clamp(0.0, 100.0);
    div()
        .id(("ring", index))
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.))
        .pt(px(6.))
        .pb(px(8.))
        .rounded(px(10.))
        .hover(|s| s.bg(rgba(HOVER)))
        .child(
            div()
                .relative()
                .size(px(52.))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let c = bounds.center();
                            let r = px(52. * 18. / 44.);
                            let w = px(52. * 4. / 44.);
                            let arc = |from: f32, to: f32| {
                                let steps = ((to - from).abs() * 64.0).ceil().max(2.0) as usize;
                                let mut b = PathBuilder::stroke(w);
                                for i in 0..=steps {
                                    let t = from + (to - from) * i as f32 / steps as f32;
                                    let a = (t * 360.0 - 90.0).to_radians();
                                    let p = point(c.x + r * a.cos(), c.y + r * a.sin());
                                    if i == 0 {
                                        b.move_to(p)
                                    } else {
                                        b.line_to(p)
                                    }
                                }
                                b.build().ok()
                            };
                            if let Some(p) = arc(0.0, 1.0) {
                                window.paint_path(p, rgba(TRACK));
                            }
                            if value > 0.5 {
                                if let Some(p) = arc(0.0, value / 100.0) {
                                    window.paint_path(p, rgb(ACCENT));
                                }
                            }
                        },
                    )
                    .size_full(),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{value:.0}%")),
                ),
        )
        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(label))
}

impl Render for Compact {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut rings: Vec<(f32, String)> = Vec::new();
        if let Some(s) = &self.sample {
            rings.push((s.cpu, "CPU".into()));
            if let Some(g) = &s.gpu {
                rings.push((g.usage as f32, g.temp.map(|t| format!("GPU · {t}°C")).unwrap_or("GPU".into())));
            }
            rings.push((
                (s.ram_used as f64 / s.ram_total.max(1) as f64 * 100.0) as f32,
                format!("RAM · {:.1} GB", s.ram_used as f64 / GB),
            ));
            if let Some(g) = s.gpu.as_ref().filter(|g| g.vram_total > 0) {
                rings.push((
                    (g.vram_used as f64 / g.vram_total as f64 * 100.0) as f32,
                    format!("VRAM · {:.1} GB", g.vram_used as f64 / GB),
                ));
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .pt(px(8.))
            .px(px(10.))
            .pb(px(10.))
            .bg(rgba(CARD))
            .text_color(rgb(TEXT))
            .font_family(if cfg!(windows) { "Segoe UI Variable Text" } else { ".SystemUIFont" })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl(px(2.))
                    .h(px(30.))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(div().size(px(7.)).rounded_full().bg(rgb(ACCENT)))
                            .child(self.host.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .children(rings.into_iter().enumerate().map(|(i, (v, l))| ring(i, v, l))),
            )
    }
}

fn backdrop() -> &'static str {
    match std::env::var("SPIKE_BG").as_deref() {
        Ok("transparent") => "transparent",
        Ok("blur") => "blur",
        Ok("opaque") => "opaque",
        _ => "mica",
    }
}

#[cfg(windows)]
fn native_tweaks(window: &Window) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };
    let Ok(handle) = window.window_handle() else { return 0 };
    let RawWindowHandle::Win32(h) = handle.as_raw() else { return 0 };
    let hwnd = h.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        let set = |attr: u32, value: i32| {
            DwmSetWindowAttribute(hwnd, attr, &value as *const i32 as *const _, 4);
        };
        set(20, 1); // DWMWA_USE_IMMERSIVE_DARK_MODE
        set(33, 2); // DWMWA_WINDOW_CORNER_PREFERENCE: round
        if backdrop() == "mica" {
            set(38, 2); // DWMWA_SYSTEMBACKDROP_TYPE: Mica
        }
    }
    hwnd as isize
}

#[cfg(not(windows))]
fn native_tweaks(_window: &Window) -> isize {
    0
}

#[cfg(windows)]
fn toggle(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsWindowVisible, SW_HIDE, SW_SHOW, ShowWindow};
    let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
    unsafe {
        let visible = IsWindowVisible(hwnd) != 0;
        ShowWindow(hwnd, if visible { SW_HIDE } else { SW_SHOW });
    }
}

#[cfg(not(target_os = "linux"))]
fn tray(cx: &mut App, hwnd: isize) {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{MouseButton as TrayButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    // 16x16 accent dot.
    let mut icon_rgba = vec![0u8; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
            if dx * dx + dy * dy <= 49.0 {
                icon_rgba[(y * 16 + x) * 4..][..4].copy_from_slice(&[0x4c, 0xc2, 0xff, 0xff]);
            }
        }
    }
    let quit = MenuItem::new("Quit", true, None);
    let quit_id = quit.id().clone();
    let menu = Menu::new();
    menu.append(&quit).unwrap();
    let icon = TrayIconBuilder::new()
        .with_tooltip("Pulsar (gpui spike)")
        .with_icon(tray_icon::Icon::from_rgba(icon_rgba, 16, 16).unwrap())
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .unwrap();
    std::mem::forget(icon);

    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(250)).await;
            while let Ok(e) = TrayIconEvent::receiver().try_recv() {
                if let TrayIconEvent::Click { button: TrayButton::Left, button_state: MouseButtonState::Up, .. } = e {
                    #[cfg(windows)]
                    toggle(hwnd);
                }
            }
            while let Ok(e) = MenuEvent::receiver().try_recv() {
                if e.id == quit_id {
                    let _ = cx.update(|cx| cx.quit());
                }
            }
        }
    })
    .detach();
    let _ = hwnd;
}

fn main() {
    let shared: Shared = Arc::default();
    spawn_sampler(shared.clone());

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(380.), px(128.)), cx);
        let background = match backdrop() {
            "opaque" => WindowBackgroundAppearance::Opaque,
            "blur" => WindowBackgroundAppearance::Blurred,
            _ => WindowBackgroundAppearance::Transparent,
        };
        let mut hwnd = 0;
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                kind: WindowKind::PopUp,
                window_background: background,
                is_resizable: false,
                is_minimizable: false,
                focus: true,
                show: true,
                app_id: Some("com.petersen.pulsar".into()),
                ..Default::default()
            },
            |window, cx| {
                hwnd = native_tweaks(window);
                cx.new(|cx| Compact::new(shared.clone(), cx))
            },
        )
        .unwrap();
        #[cfg(not(target_os = "linux"))]
        tray(cx, hwnd);
        let _ = hwnd;
    });
}
