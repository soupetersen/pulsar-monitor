//! Colours and sizes, matching the tokens of the WebView UI (app.css).

use gpui::{Pixels, Rgba, rgb, rgba};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub text: Rgba,
    pub muted: Rgba,
    pub card: Rgba,
    pub inset: Rgba,
    pub chip: Rgba,
    pub track: Rgba,
    pub hover: Rgba,
    pub accent: Rgba,
    /// Tooltip background: opaque, since it floats over the content.
    pub tip: Rgba,
}

impl Theme {
    pub fn get(mode: Mode) -> Self {
        // Linux has no system backdrop behind the window, so the card is
        // drawn (almost) opaque there.
        let linux = cfg!(target_os = "linux");
        match mode {
            Mode::Dark => Theme {
                text: rgb(0xf3f4f6),
                muted: rgb(0xb4b9c2),
                card: rgba(if linux { 0x1c1d22f5 } else { 0x1c1d228c }),
                inset: rgba(0xffffff0d),
                chip: rgba(0xffffff12),
                track: rgba(0xffffff1f),
                hover: rgba(0xffffff17),
                accent: rgb(0x4cc2ff),
                tip: rgb(0x2b2c31),
            },
            Mode::Light => Theme {
                text: rgb(0x111111),
                muted: rgb(0x565b64),
                card: rgba(if linux { 0xfcfcfdfa } else { 0xfcfcfde6 }),
                inset: rgba(0x0000000a),
                chip: rgba(0x0000000d),
                track: rgba(0x00000017),
                hover: rgba(0x0000000f),
                accent: rgb(0x111111),
                tip: rgb(0xffffff),
            },
        }
    }
}

/// The system UI font.
pub fn font_family() -> &'static str {
    if cfg!(windows) {
        "Segoe UI Variable Text"
    } else if cfg!(target_os = "macos") {
        ".SystemUIFont"
    } else {
        "Cantarell"
    }
}

/// GNOME's text scaling factor (`org.gnome.desktop.interface
/// text-scaling-factor`, "Large text"). GTK applies it to the whole UI and
/// gpui doesn't, so every size goes through `u()` to follow it.
fn ui_scale() -> f32 {
    use std::sync::OnceLock;
    static SCALE: OnceLock<f32> = OnceLock::new();
    *SCALE.get_or_init(|| {
        if let Some(v) = std::env::var("PULSAR_SCALE").ok().and_then(|v| v.parse().ok()) {
            return v;
        }
        #[cfg(target_os = "linux")]
        if let Ok(out) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "text-scaling-factor"])
            .output()
        {
            if let Ok(v) = String::from_utf8_lossy(&out.stdout).trim().parse::<f32>() {
                if (0.5..=3.0).contains(&v) {
                    return v;
                }
            }
        }
        1.0
    })
}

/// A size in logical pixels, scaled by the desktop's text scaling.
pub fn u(v: f32) -> Pixels {
    gpui::px(v * ui_scale())
}
