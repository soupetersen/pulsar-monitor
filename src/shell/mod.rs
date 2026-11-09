//! Everything around the views: the window and its native behaviour, the
//! tray, single instance, start at login, settings, and the sampler thread
//! feeding the UI.
//!
//! Background threads (tray, single instance, setup prompts) never touch
//! the UI: they send an `Action`, handled on the UI thread by `handle`.

mod autostart;
mod labels;
mod settings;
mod single_instance;
mod tray;
mod window;

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{
    App, AppContext, Application, Bounds, Entity, Global, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    point, size,
};

use crate::monitor::{self, Panel, SharedView, Stats, View};
use crate::ui::{Widget, theme::u};
use settings::Settings;

pub const APP_ID: &str = "com.petersen.pulsar";

pub enum Action {
    Toggle,
    Show,
    ToggleOnTop,
    ToggleAutostart,
    #[cfg_attr(not(windows), allow(dead_code))]
    ToggleCpuTemp,
    /// The CPU temperature setup finished (Windows): refresh the menu.
    #[cfg_attr(not(windows), allow(dead_code))]
    CpuTempChanged,
    Quit,
}

/// The latest sample. Views observe it and redraw when it changes.
pub struct StatsStore {
    pub latest: Option<Stats>,
}

struct Shell {
    view: SharedView,
    settings: Settings,
    stats: Entity<StatsStore>,
    window: Option<WindowHandle<Widget>>,
    visible: bool,
    tray: Option<tray::Tray>,
    actions: UnboundedSender<Action>,
}

impl Global for Shell {}

pub fn run() {
    let (actions, mut action_rx) = unbounded::<Action>();
    let show_tx = actions.clone();
    if !single_instance::acquire(move || {
        let _ = show_tx.unbounded_send(Action::Show);
    }) {
        return;
    }

    let view: SharedView = Arc::new(Mutex::new(View { visible: true, expanded: false, panel: Panel::None }));
    let (stats_tx, mut stats_rx) = unbounded::<Stats>();
    monitor::spawn(view.clone(), move |stats| {
        let _ = stats_tx.unbounded_send(stats);
    });

    Application::new().run(move |cx: &mut App| {
        let settings = Settings::load();
        let stats = cx.new(|_| StatsStore { latest: None });
        let tray = tray::Tray::new(actions.clone(), checks(&settings));
        cx.set_global(Shell { view, settings, stats: stats.clone(), window: None, visible: true, tray, actions });
        open_window(cx);

        cx.spawn(async move |cx| {
            while let Some(sample) = stats_rx.next().await {
                let _ = stats.update(cx, |store, cx| {
                    store.latest = Some(sample);
                    cx.notify();
                });
            }
        })
        .detach();
        cx.spawn(async move |cx| {
            while let Some(action) = action_rx.next().await {
                if cx.update(|cx| handle(action, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    });
}

fn checks(settings: &Settings) -> tray::Checks {
    tray::Checks { on_top: settings.on_top, autostart: autostart::is_enabled(), cpu_temp: cpu_temp_check() }
}

#[cfg(windows)]
fn cpu_temp_check() -> Option<(bool, bool)> {
    use monitor::cpu_sensor;
    cpu_sensor::vendor()?;
    let pawnio = cpu_sensor::pawnio_installed();
    Some((pawnio && cpu_sensor::enabled(), pawnio))
}

#[cfg(not(windows))]
fn cpu_temp_check() -> Option<(bool, bool)> {
    None
}

fn refresh_tray(cx: &mut App) {
    let shell = cx.global::<Shell>();
    if let Some(tray) = &shell.tray {
        tray.set(checks(&shell.settings));
    }
}

fn handle(action: Action, cx: &mut App) {
    match action {
        Action::Toggle => {
            let visible = cx.global::<Shell>().visible;
            set_visible(cx, !visible);
        }
        Action::Show => set_visible(cx, true),
        Action::ToggleOnTop => {
            let shell = cx.global_mut::<Shell>();
            shell.settings.on_top = !shell.settings.on_top;
            shell.settings.save();
            let on = shell.settings.on_top;
            if let Some(handle) = shell.window {
                let _ = handle.update(cx, |_, window, _| window::set_on_top(window, on));
            }
            refresh_tray(cx);
        }
        Action::ToggleAutostart => {
            autostart::set(!autostart::is_enabled());
            refresh_tray(cx);
        }
        Action::ToggleCpuTemp => {
            #[cfg(windows)]
            {
                use monitor::cpu_sensor;
                // The UAC prompt and schtasks take a while: off the UI thread.
                let tx = cx.global::<Shell>().actions.clone();
                std::thread::spawn(move || {
                    if cpu_sensor::pawnio_installed() {
                        cpu_sensor::request_toggle(!cpu_sensor::enabled());
                    }
                    let _ = tx.unbounded_send(Action::CpuTempChanged);
                });
            }
        }
        Action::CpuTempChanged => refresh_tray(cx),
        Action::Quit => {
            save_position(cx);
            cx.quit();
        }
    }
}

fn open_window(cx: &mut App) {
    let shell = cx.global::<Shell>();
    let on_top = shell.settings.on_top;
    let mode = shell.settings.theme;
    let stats = shell.stats.clone();
    let win_size = size(u(380.), u(128.));
    let bounds = match shell.settings.position {
        Some((x, y)) => Bounds { origin: point(gpui::px(x), gpui::px(y)), size: win_size },
        None => Bounds::centered(None, win_size, cx),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        // No taskbar entry; always on top is set natively (see window.rs).
        kind: WindowKind::PopUp,
        window_background: window::background(),
        is_resizable: false,
        is_minimizable: false,
        focus: true,
        show: true,
        app_id: Some(APP_ID.into()),
        ..Default::default()
    };
    let handle = cx
        .open_window(options, |window, cx| {
            window::apply_native_style(window, on_top);
            cx.new(|cx| Widget::new(stats, mode, cx))
        })
        .expect("failed to open the window");
    cx.global_mut::<Shell>().window = Some(handle);
}

fn save_position(cx: &mut App) {
    let Some(handle) = cx.global::<Shell>().window else { return };
    // Wayland doesn't tell apps where their window is.
    if window::position_is_known() {
        if let Ok(origin) = handle.update(cx, |_, window, _| window.bounds().origin) {
            let shell = cx.global_mut::<Shell>();
            shell.settings.position = Some((f32::from(origin.x), f32::from(origin.y)));
            shell.settings.save();
        }
    }
}

fn set_visible(cx: &mut App, visible: bool) {
    if !visible {
        save_position(cx);
    }
    let shell = cx.global_mut::<Shell>();
    shell.visible = visible;
    shell.view.lock().unwrap().visible = visible;
    let Some(handle) = shell.window else { return };
    if visible {
        if window::can_hide() {
            let _ = handle.update(cx, |_, window, _| window::show(window));
        } else {
            // Wayland can't hide a window or bring a minimized one back, so
            // the widget is minimized and replaced by a new window.
            open_window(cx);
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    } else {
        let _ = handle.update(cx, |_, window, _| window::hide(window));
    }
}
