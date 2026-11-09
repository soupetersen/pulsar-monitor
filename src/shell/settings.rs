//! Settings kept between runs, as JSON in the user's config folder:
//! `%APPDATA%\com.petersen.pulsar` on Windows, `~/.config/com.petersen.pulsar`
//! on Linux and `~/Library/Application Support/com.petersen.pulsar` on macOS.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::APP_ID;
use crate::ui::theme::Mode;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub theme: Mode,
    pub on_top: bool,
    /// Top-left corner of the window, where the OS lets apps know it (not
    /// on Wayland): in physical screen pixels on Windows (monitors can have
    /// different scales), in gpui's logical pixels elsewhere.
    pub position: Option<(f32, f32)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: Mode::Dark, on_top: true, position: None }
    }
}

fn dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join(APP_ID))
}

fn file() -> Option<PathBuf> {
    dir().map(|d| d.join("settings.json"))
}

impl Settings {
    pub fn load() -> Self {
        file()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let (Some(dir), Some(file)) = (dir(), file()) else { return };
        let _ = std::fs::create_dir_all(dir);
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(file, json);
        }
    }
}
