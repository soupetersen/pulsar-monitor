//! The UI icons, compiled into the binary and served to gpui's `svg()`.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        &[$((concat!("ui/", $name, ".svg"), include_bytes!(concat!("../../assets/ui/", $name, ".svg")))),*]
    };
}

const ICONS: &[(&str, &[u8])] = icons!(
    "bolt",
    "chevron",
    "collapse",
    "download",
    "expand",
    "minimize",
    "moon",
    "pin",
    "sun",
    "thermometer",
);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS.iter().find(|(p, _)| *p == path).map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS.iter().filter(|(p, _)| p.starts_with(path)).map(|(p, _)| (*p).into()).collect())
    }
}

/// Icon paths for `svg().path(..)`.
pub mod icon {
    pub const BOLT: &str = "ui/bolt.svg";
    pub const CHEVRON: &str = "ui/chevron.svg";
    pub const COLLAPSE: &str = "ui/collapse.svg";
    pub const DOWNLOAD: &str = "ui/download.svg";
    pub const EXPAND: &str = "ui/expand.svg";
    pub const MINIMIZE: &str = "ui/minimize.svg";
    pub const MOON: &str = "ui/moon.svg";
    pub const PIN: &str = "ui/pin.svg";
    pub const SUN: &str = "ui/sun.svg";
    pub const THERMOMETER: &str = "ui/thermometer.svg";
}
