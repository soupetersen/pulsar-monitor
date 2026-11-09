// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        #[cfg(windows)]
        Some(pulsar_lib::cpu_sensor::HELPER_ARG) => std::process::exit(pulsar_lib::cpu_sensor::run_helper()),
        #[cfg(windows)]
        Some(pulsar_lib::cpu_sensor::ENABLE_ARG) => {
            std::process::exit(pulsar_lib::cpu_sensor::install(args.get(2).map(String::as_str).unwrap_or_default()))
        }
        #[cfg(windows)]
        Some(pulsar_lib::cpu_sensor::DISABLE_ARG) => std::process::exit(pulsar_lib::cpu_sensor::uninstall()),
        Some("--gpu-info") => pulsar_lib::print_gpu_info(),
        _ => pulsar_lib::run(),
    }
}
