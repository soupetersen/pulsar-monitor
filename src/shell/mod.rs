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
mod updater;
mod window;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{
    App, AppContext, Application, Bounds, Entity, Global, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    point, size,
};

use crate::monitor::{self, Panel, SharedView, Stats, View};
use crate::ui::Widget;
use crate::ui::theme::{Mode, u};
use settings::Settings;

pub const APP_ID: &str = "com.petersen.pulsar";

pub enum Action {
    Toggle,
    Show,
    Hide,
    ToggleOnTop,
    ToggleAutostart,
    #[cfg_attr(not(windows), allow(dead_code))]
    ToggleCpuTemp,
    /// The CPU temperature setup finished (Windows): refresh the menu.
    #[cfg_attr(not(windows), allow(dead_code))]
    CpuTempChanged,
    /// Linux: run the CPU power setup (password prompt).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    EnablePower,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    PowerChanged,
    UpdateFound(updater::Update),
    InstallUpdate,
    UpdateInstalled(updater::Restart),
    UpdateFailed,
    Quit,
}

/// A newer version found by the updater, waiting for the user.
#[derive(Clone)]
pub struct UpdateState {
    update: updater::Update,
    pub busy: bool,
    pub failed: bool,
}

impl UpdateState {
    pub fn version(&self) -> &str {
        &self.update.version
    }
}

/// CPU sensors that need a one-time setup before they can be read.
#[derive(Clone, Copy, Default)]
pub struct Sensors {
    /// The CPU temperature can be turned on but isn't (Windows). The value
    /// is whether the PawnIO driver it needs is installed.
    pub temp_setup: Option<bool>,
    pub temp_busy: bool,
    /// The CPU power counter is there but root-only (Linux).
    pub power_setup: bool,
    pub power_busy: bool,
}

fn sensors_now() -> Sensors {
    #[cfg(target_os = "linux")]
    let power_setup = monitor::power_setup_needed();
    #[cfg(not(target_os = "linux"))]
    let power_setup = false;
    let temp_setup = cpu_temp_check().and_then(|(enabled, pawnio)| (!enabled).then_some(pawnio));
    Sensors { temp_setup, temp_busy: false, power_setup, power_busy: false }
}

/// Samples kept for the usage history graphs.
const HISTORY: usize = 30;

/// Value changes animate in a few discrete frames, like the WebView UI
/// (`steps(8)` over 0.4 s): it reads as motion while redrawing only 8 times
/// per sample.
const ANIM_STEPS: u32 = 8;
const ANIM_STEP: Duration = Duration::from_millis(50);

/// The latest sample and the CPU and GPU usage history. Views observe it
/// and redraw when it changes.
pub struct StatsStore {
    pub latest: Option<Stats>,
    /// The sample before, which bars and rings animate from.
    pub prev: Option<Stats>,
    changed: Instant,
    pub cpu_hist: Vec<f32>,
    pub gpu_hist: Vec<f32>,
}

impl StatsStore {
    fn push(&mut self, sample: Stats) {
        let push = |list: &mut Vec<f32>, v: f32| {
            if list.len() >= HISTORY {
                list.remove(0);
            }
            list.push(v);
        };
        push(&mut self.cpu_hist, sample.cpu.usage);
        push(&mut self.gpu_hist, sample.gpu.as_ref().map_or(0.0, |g| g.usage));
        self.prev = self.latest.replace(sample);
        self.changed = Instant::now();
    }

    /// How far the animation from `prev` to `latest` is, from 0 to 1, in
    /// `ANIM_STEPS` steps.
    pub fn progress(&self) -> f32 {
        let t = self.changed.elapsed().as_secs_f32() / (ANIM_STEP.as_secs_f32() * ANIM_STEPS as f32);
        (t.min(1.0) * ANIM_STEPS as f32).floor() / ANIM_STEPS as f32
    }
}

struct Shell {
    view: SharedView,
    settings: Settings,
    stats: Entity<StatsStore>,
    window: Option<WindowHandle<Widget>>,
    visible: bool,
    tray: Option<tray::Tray>,
    sensors: Sensors,
    update: Option<UpdateState>,
    /// Wakes the updater for an early check.
    check_updates: std::sync::mpsc::Sender<()>,
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

    Application::new().with_assets(crate::ui::Assets).run(move |cx: &mut App| {
        let settings = Settings::load();
        let stats = cx.new(|_| StatsStore { latest: None, prev: None, changed: Instant::now(), cpu_hist: Vec::new(), gpu_hist: Vec::new() });
        let tray = tray::Tray::new(actions.clone(), checks(&settings));
        let (check_updates, nudge) = std::sync::mpsc::channel();
        let found_tx = actions.clone();
        updater::spawn(move |update| drop(found_tx.unbounded_send(Action::UpdateFound(update))), nudge);
        cx.set_global(Shell { view, settings, stats: stats.clone(), window: None, visible: true, tray, sensors: sensors_now(), update: None, check_updates, actions });
        open_window(cx);

        cx.spawn(async move |cx| {
            while let Some(sample) = stats_rx.next().await {
                let _ = stats.update(cx, |store, cx| {
                    store.push(sample);
                    cx.notify();
                });
                for _ in 0..ANIM_STEPS {
                    cx.background_executor().timer(ANIM_STEP).await;
                    let _ = stats.update(cx, |_, cx| cx.notify());
                }
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
        Action::Hide => set_visible(cx, false),
        Action::ToggleOnTop => {
            let shell = cx.global_mut::<Shell>();
            shell.settings.on_top = !shell.settings.on_top;
            shell.settings.save();
            let on = shell.settings.on_top;
            if let Some(handle) = shell.window {
                let _ = handle.update(cx, |_, window, cx| {
                    window::set_on_top(window, on);
                    // The pin button shows the state.
                    cx.notify();
                });
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
                let shell = cx.global_mut::<Shell>();
                let tx = shell.actions.clone();
                shell.sensors.temp_busy = true;
                notify_window(cx);
                std::thread::spawn(move || {
                    if cpu_sensor::pawnio_installed() {
                        cpu_sensor::request_toggle(!cpu_sensor::enabled());
                    }
                    let _ = tx.unbounded_send(Action::CpuTempChanged);
                });
            }
        }
        Action::CpuTempChanged => {
            let shell = cx.global_mut::<Shell>();
            shell.sensors.temp_busy = false;
            shell.sensors.temp_setup = sensors_now().temp_setup;
            refresh_tray(cx);
            notify_window(cx);
        }
        Action::EnablePower => {
            #[cfg(target_os = "linux")]
            {
                let shell = cx.global_mut::<Shell>();
                if shell.sensors.power_busy {
                    return;
                }
                shell.sensors.power_busy = true;
                let tx = shell.actions.clone();
                notify_window(cx);
                std::thread::spawn(move || {
                    monitor::enable_power();
                    let _ = tx.unbounded_send(Action::PowerChanged);
                });
            }
        }
        Action::PowerChanged => {
            let shell = cx.global_mut::<Shell>();
            shell.sensors.power_busy = false;
            shell.sensors.power_setup = sensors_now().power_setup;
            notify_window(cx);
        }
        Action::UpdateFound(update) => {
            let shell = cx.global_mut::<Shell>();
            if shell.update.as_ref().is_some_and(|u| u.busy) {
                return;
            }
            shell.update = Some(UpdateState { update, busy: false, failed: false });
            notify_window(cx);
        }
        Action::InstallUpdate => {
            let shell = cx.global_mut::<Shell>();
            let Some(state) = shell.update.as_mut().filter(|u| !u.busy) else { return };
            state.busy = true;
            state.failed = false;
            let update = state.update.clone();
            let tx = shell.actions.clone();
            notify_window(cx);
            std::thread::spawn(move || {
                let action = match updater::install(&update) {
                    Ok(restart) => Action::UpdateInstalled(restart),
                    Err(_) => Action::UpdateFailed,
                };
                let _ = tx.unbounded_send(action);
            });
        }
        Action::UpdateInstalled(restart) => {
            save_position(cx);
            if let updater::Restart::Relaunch(path) = restart {
                updater::relaunch(&path);
            }
            cx.quit();
        }
        Action::UpdateFailed => {
            if let Some(state) = cx.global_mut::<Shell>().update.as_mut() {
                state.busy = false;
                state.failed = true;
            }
            notify_window(cx);
        }
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
    // The widget resizes the window to its content on the first frame.
    let win_size = size(u(372.), u(128.));
    let position = shell.settings.position;
    let bounds = match position.filter(|_| !window::NATIVE_POSITION) {
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
        // Windows: shown once it is in its saved place (below).
        show: !window::NATIVE_POSITION,
        app_id: Some(APP_ID.into()),
        ..Default::default()
    };
    let handle = cx
        .open_window(options, |window, cx| {
            window::apply_native_style(window, on_top, mode == Mode::Dark);
            if window::NATIVE_POSITION {
                if let Some(position) = position {
                    window::set_position(window, position);
                }
                window::show(window);
            }
            cx.new(|cx| Widget::new(stats, mode, cx))
        })
        .expect("failed to open the window");
    cx.global_mut::<Shell>().window = Some(handle);
}

fn save_position(cx: &mut App) {
    let Some(handle) = cx.global::<Shell>().window else { return };
    // Wayland doesn't tell apps where their window is.
    if window::position_is_known() {
        if let Ok(Some(position)) = handle.update(cx, |_, window, _| window::position(window)) {
            let shell = cx.global_mut::<Shell>();
            shell.settings.position = Some(position);
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
    if visible {
        // Shown after a while: a good moment to look for an update.
        let _ = shell.check_updates.send(());
    }
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

// Called by the views. They run inside a window update, so anything that
// touches the window again goes through the action channel.

/// Redraws the widget (state it reads from the shell changed).
fn notify_window(cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().window {
        let _ = handle.update(cx, |_, _, cx| cx.notify());
    }
}

fn send(action: Action, cx: &App) {
    let _ = cx.global::<Shell>().actions.unbounded_send(action);
}

pub fn hide(cx: &App) {
    send(Action::Hide, cx);
}

pub fn toggle_on_top(cx: &App) {
    send(Action::ToggleOnTop, cx);
}

pub fn on_top(cx: &App) -> bool {
    cx.global::<Shell>().settings.on_top
}

pub fn set_theme(mode: Mode, window: &mut gpui::Window, cx: &mut App) {
    let shell = cx.global_mut::<Shell>();
    shell.settings.theme = mode;
    shell.settings.save();
    window::set_dark(window, mode == Mode::Dark);
}

/// Resizes the window to the widget's content, keeping it on screen.
pub fn fit_window(size: gpui::Size<gpui::Pixels>, window: &mut gpui::Window, cx: &App) {
    window::fit(window, size, cx);
}

/// Whether the widget is expanded, and its open process list.
pub fn view_state(cx: &App) -> (bool, Panel) {
    let view = cx.global::<Shell>().view.lock().unwrap();
    (view.expanded, view.panel)
}

pub fn set_view(expanded: bool, panel: Panel, cx: &App) {
    let mut view = cx.global::<Shell>().view.lock().unwrap();
    view.expanded = expanded;
    view.panel = panel;
}

pub fn sensors(cx: &App) -> Sensors {
    cx.global::<Shell>().sensors
}

/// The CPU temperature button: turns it on (UAC prompt), or opens the
/// PawnIO website when the driver it needs is missing.
pub fn enable_cpu_temp(cx: &App) {
    match sensors(cx).temp_setup {
        Some(true) => send(Action::ToggleCpuTemp, cx),
        Some(false) => open_url("https://pawnio.eu/"),
        None => {}
    }
}

pub fn enable_power(cx: &App) {
    send(Action::EnablePower, cx);
}

fn open_url(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

pub fn update(cx: &App) -> Option<UpdateState> {
    cx.global::<Shell>().update.clone()
}

pub fn install_update(cx: &App) {
    send(Action::InstallUpdate, cx);
}
