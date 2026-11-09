pub mod cpu_sensor;
mod gpu;
mod gpu_kmt;
mod pdh;
mod sampler;

use std::sync::{Arc, Mutex};

use sampler::{Panel, SharedView, View};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_window_state::StateFlags;

struct TrayItems {
    on_top: CheckMenuItem<Wry>,
    cpu_temp: CheckMenuItem<Wry>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CpuTempStatus {
    /// AMD or Intel CPU (the kinds the sensor supports).
    supported: bool,
    /// PawnIO driver installed.
    pawnio: bool,
    /// Helper task registered.
    enabled: bool,
}

fn cpu_temp_status_now() -> CpuTempStatus {
    let supported = cpu_sensor::vendor().is_some();
    let pawnio = supported && cpu_sensor::pawnio_installed();
    CpuTempStatus { supported, pawnio, enabled: pawnio && cpu_sensor::enabled() }
}

#[tauri::command]
async fn cpu_temp_status() -> CpuTempStatus {
    cpu_temp_status_now()
}

/// Enables or disables the CPU temperature helper (shows a UAC prompt).
#[tauri::command]
async fn set_cpu_temp(app: AppHandle, enable: bool) -> CpuTempStatus {
    toggle_cpu_temp(&app, enable)
}

fn toggle_cpu_temp(app: &AppHandle, enable: bool) -> CpuTempStatus {
    cpu_sensor::request_toggle(enable);
    let status = cpu_temp_status_now();
    let _ = app.state::<TrayItems>().cpu_temp.set_checked(status.enabled);
    let _ = app.emit("cpu-temp-changed", status.clone());
    status
}

/// Opens the PawnIO website (where the driver installer is) in the browser.
#[tauri::command]
fn open_pawnio_site() {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", "https://pawnio.eu/"])
        .creation_flags(0x0800_0000)
        .spawn();
}

/// The UI tells the sampler what it is showing, so nothing extra is collected.
#[tauri::command]
fn set_view(view: State<SharedView>, expanded: bool, panel: Panel) {
    let mut v = view.lock().unwrap();
    v.expanded = expanded;
    v.panel = panel;
}

#[tauri::command]
fn hide_to_tray(app: AppHandle) {
    set_visible(&app, false);
}

#[tauri::command]
fn is_on_top(app: AppHandle) -> bool {
    app.get_webview_window("main").and_then(|w| w.is_always_on_top().ok()).unwrap_or(false)
}

#[tauri::command]
fn set_on_top(app: AppHandle, on: bool) {
    apply_on_top(&app, on);
}

fn apply_on_top(app: &AppHandle, on: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_always_on_top(on);
    }
    let _ = app.state::<TrayItems>().on_top.set_checked(on);
    let _ = app.emit("on-top-changed", on);
}

fn set_visible(app: &AppHandle, visible: bool) {
    if let Some(w) = app.get_webview_window("main") {
        if visible {
            set_memory_level(&w, false);
            let _ = w.show();
            let _ = w.set_focus();
        } else {
            let _ = w.hide();
            set_memory_level(&w, true);
        }
    }
    app.state::<SharedView>().lock().unwrap().visible = visible;
}

/// While hidden, ask WebView2 to release caches and GPU buffers
/// (COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW); back to normal when shown.
fn set_memory_level(window: &tauri::WebviewWindow, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL, ICoreWebView2_19,
    };
    use windows_core::Interface;

    let _ = window.with_webview(move |webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        let Ok(core19) = core.cast::<ICoreWebView2_19>() else { return };
        let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
        let _ = core19.SetMemoryUsageTargetLevel(level);
    });
}

fn toggle_visible(app: &AppHandle) {
    let visible = app.state::<SharedView>().lock().unwrap().visible;
    set_visible(app, !visible);
}

/// Tray menu labels: English by default, Portuguese when the system language is Portuguese.
struct TrayLabels {
    toggle: &'static str,
    on_top: &'static str,
    autostart: &'static str,
    cpu_temp: &'static str,
    cpu_temp_needs_pawnio: &'static str,
    quit: &'static str,
}

fn tray_labels() -> TrayLabels {
    let pt = sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("pt"));
    if pt {
        TrayLabels {
            toggle: "Mostrar / ocultar",
            on_top: "Sempre no topo",
            autostart: "Iniciar com o Windows",
            cpu_temp: "Temperatura da CPU (requer admin)",
            cpu_temp_needs_pawnio: "Temperatura da CPU (requer PawnIO)",
            quit: "Sair",
        }
    } else {
        TrayLabels {
            toggle: "Show / hide",
            on_top: "Always on top",
            autostart: "Start with Windows",
            cpu_temp: "CPU temperature (needs admin)",
            cpu_temp_needs_pawnio: "CPU temperature (needs PawnIO)",
            quit: "Quit",
        }
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let labels = tray_labels();
    let show = MenuItem::with_id(app, "toggle", labels.toggle, true, None::<&str>)?;
    let on_top = CheckMenuItem::with_id(app, "on_top", labels.on_top, true, true, None::<&str>)?;
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart = CheckMenuItem::with_id(app, "autostart", labels.autostart, true, autostart_on, None::<&str>)?;
    // CPU temperature (AMD and Intel): needs the PawnIO driver and a one-time UAC prompt.
    let cpu_temp_supported = cpu_sensor::vendor().is_some();
    let pawnio = cpu_temp_supported && cpu_sensor::pawnio_installed();
    let cpu_temp = CheckMenuItem::with_id(
        app,
        "cpu_temp",
        if pawnio { labels.cpu_temp } else { labels.cpu_temp_needs_pawnio },
        pawnio,
        pawnio && cpu_sensor::enabled(),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = if cpu_temp_supported {
        Menu::with_items(app, &[&show, &on_top, &autostart, &cpu_temp, &sep, &quit])?
    } else {
        Menu::with_items(app, &[&show, &on_top, &autostart, &sep, &quit])?
    };

    app.manage(TrayItems { on_top: on_top.clone(), cpu_temp: cpu_temp.clone() });

    let autostart_item = autostart.clone();
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("app icon missing"))
        .tooltip("Pulsar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "toggle" => toggle_visible(app),
            "on_top" => {
                let on = !is_on_top(app.clone());
                apply_on_top(app, on);
            }
            "autostart" => {
                let launcher = app.autolaunch();
                let enabled = launcher.is_enabled().unwrap_or(false);
                let _ = if enabled { launcher.disable() } else { launcher.enable() };
                let _ = autostart_item.set_checked(launcher.is_enabled().unwrap_or(false));
            }
            "cpu_temp" => {
                // The UAC prompt and schtasks take a while: don't block the UI thread.
                let app = app.clone();
                std::thread::spawn(move || {
                    cpu_sensor::log("tray: CPU temperature item clicked");
                    toggle_cpu_temp(&app, !cpu_sensor::enabled());
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_visible(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let view: SharedView = Arc::new(Mutex::new(View { visible: true, expanded: false, panel: Panel::None }));

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::new().with_state_flags(StateFlags::POSITION).build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage(view.clone())
        .invoke_handler(tauri::generate_handler![
            set_view,
            hide_to_tray,
            is_on_top,
            set_on_top,
            cpu_temp_status,
            set_cpu_temp,
            open_pawnio_site
        ])
        .setup(move |app| {
            build_tray(app)?;
            sampler::spawn(app.handle().clone(), view.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing (e.g. Alt+F4) just sends the widget to the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                set_visible(window.app_handle(), false);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// `--gpu-info`: prints what each GPU reports (diagnostics).
pub fn print_gpu_info() {
    for a in gpu_kmt::adapters() {
        println!(
            "D3DKMT  {:<40} luid {}  vram {:.1} GB  {:?}",
            a.name,
            a.luid,
            a.vram_total as f64 / (1u64 << 30) as f64,
            a.perf()
        );
    }
    if let Some(main) = gpu_kmt::main_adapter() {
        println!("main adapter: {}", main.name);
    }
    match gpu::NvGpu::new().and_then(|nv| nv.sample().map(|s| (nv.name.clone(), s))) {
        Some((name, s)) => println!(
            "NVML    {name:<40} usage {}%  temp {:?}  power {:?} W  fan {:?}%",
            s.usage, s.temp_c, s.power_w, s.fan_pct
        ),
        None => println!("NVML    not available"),
    }
}
