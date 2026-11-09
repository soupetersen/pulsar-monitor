//! Auto-update: checks the latest GitHub release shortly after startup and
//! every few hours. When a newer, correctly signed version exists, the UI
//! shows an update icon; clicking it downloads, installs and restarts.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// The update found by the last check, waiting for the user to install it.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
}

pub fn spawn_checks(app: AppHandle) {
    std::thread::Builder::new()
        .name("updater".into())
        .spawn(move || {
            std::thread::sleep(FIRST_CHECK);
            loop {
                tauri::async_runtime::block_on(check(&app));
                std::thread::sleep(CHECK_EVERY);
            }
        })
        .expect("failed to start updater thread");
}

async fn check(app: &AppHandle) {
    let Ok(updater) = app.updater() else { return };
    // Errors (offline, no release for this platform yet...) are ignored: we just try again later.
    if let Ok(Some(update)) = updater.check().await {
        let info = UpdateInfo { version: update.version.clone() };
        *app.state::<Pending>().0.lock().unwrap() = Some(update);
        let _ = app.emit("update-available", info);
    }
}

/// The pending update, if a check already found one (the UI asks on startup).
#[tauri::command]
pub fn update_status(app: AppHandle) -> Option<UpdateInfo> {
    app.state::<Pending>().0.lock().unwrap().as_ref().map(|u| UpdateInfo { version: u.version.clone() })
}

/// Downloads and installs the pending update, then restarts Pulsar.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app.state::<Pending>().0.lock().unwrap().take().ok_or("no update pending")?;
    if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
        // Keep it so the user can try again.
        *app.state::<Pending>().0.lock().unwrap() = Some(update);
        return Err(e.to_string());
    }
    app.restart();
}
