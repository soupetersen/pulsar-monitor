// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Pulsar: a small system monitor widget.
//!
//! - `core/`: data collection, independent of the UI (module `monitor`).
//! - `shell/`: window behaviour, tray, single instance, settings.
//! - `ui/`: the gpui views.

#[path = "core/mod.rs"]
mod monitor;
mod shell;
mod ui;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        #[cfg(windows)]
        Some(monitor::cpu_sensor::HELPER_ARG) => std::process::exit(monitor::cpu_sensor::run_helper()),
        #[cfg(windows)]
        Some(monitor::cpu_sensor::ENABLE_ARG) => {
            std::process::exit(monitor::cpu_sensor::install(args.get(2).map(String::as_str).unwrap_or_default()))
        }
        #[cfg(windows)]
        Some(monitor::cpu_sensor::DISABLE_ARG) => std::process::exit(monitor::cpu_sensor::uninstall()),
        Some("--gpu-info") => monitor::print_gpu_info(),
        _ => shell::run(),
    }
}
