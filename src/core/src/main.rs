// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pulsar_lib::cpu_sensor;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some(cpu_sensor::HELPER_ARG) => std::process::exit(cpu_sensor::run_helper()),
        Some(cpu_sensor::ENABLE_ARG) => {
            std::process::exit(cpu_sensor::install(args.get(2).map(String::as_str).unwrap_or_default()))
        }
        Some(cpu_sensor::DISABLE_ARG) => std::process::exit(cpu_sensor::uninstall()),
        _ => pulsar_lib::run(),
    }
}
