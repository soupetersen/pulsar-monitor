//! Tray (Windows), menu bar (macOS) or StatusNotifierItem (Linux) icon.
//! Left click shows or hides the widget; the menu has always on top, start
//! at login, CPU temperature (Windows) and quit. Clicks become `Action`s.

use futures::channel::mpsc::UnboundedSender;

use super::{Action, labels};

const ICON_PNG: &[u8] = include_bytes!("../../assets/icons/32x32.png");

/// What the menu's checkmarks show.
#[derive(Clone, Copy)]
pub struct Checks {
    pub on_top: bool,
    pub autostart: bool,
    /// Windows: `Some((enabled, pawnio_installed))` when the CPU supports it.
    pub cpu_temp: Option<(bool, bool)>,
}

fn icon_rgba() -> (Vec<u8>, u32, u32) {
    let img = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
        .map(|i| i.to_rgba8())
        .unwrap_or_else(|_| image::RgbaImage::new(1, 1));
    let (w, h) = img.dimensions();
    (img.into_raw(), w, h)
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    use super::*;

    pub struct Tray {
        _icon: TrayIcon,
        on_top: CheckMenuItem,
        autostart: CheckMenuItem,
        cpu_temp: Option<CheckMenuItem>,
    }

    impl Tray {
        pub fn new(tx: UnboundedSender<Action>, checks: Checks) -> Option<Self> {
            let l = labels::get();
            let toggle = MenuItem::with_id("toggle", l.toggle, true, None);
            let on_top = CheckMenuItem::with_id("on_top", l.on_top, true, checks.on_top, None);
            let autostart = CheckMenuItem::with_id("autostart", l.autostart, true, checks.autostart, None);
            let cpu_temp = checks.cpu_temp.map(|(enabled, pawnio)| {
                let label = if pawnio { l.cpu_temp } else { l.cpu_temp_needs_pawnio };
                CheckMenuItem::with_id("cpu_temp", label, pawnio, enabled, None)
            });
            let quit = MenuItem::with_id("quit", l.quit, true, None);
            let menu = Menu::new();
            menu.append_items(&[&toggle, &on_top, &autostart]).ok()?;
            if let Some(item) = &cpu_temp {
                menu.append(item).ok()?;
            }
            menu.append_items(&[&PredefinedMenuItem::separator(), &quit]).ok()?;

            let (rgba, w, h) = icon_rgba();
            let icon = TrayIconBuilder::new()
                .with_tooltip("Pulsar")
                .with_icon(tray_icon::Icon::from_rgba(rgba, w, h).ok()?)
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false)
                .build()
                .ok()?;

            let menu_tx = tx.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let action = match e.id.as_ref() {
                    "toggle" => Action::Toggle,
                    "on_top" => Action::ToggleOnTop,
                    "autostart" => Action::ToggleAutostart,
                    "cpu_temp" => Action::ToggleCpuTemp,
                    "quit" => Action::Quit,
                    _ => return,
                };
                let _ = menu_tx.unbounded_send(action);
            }));
            TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                    let _ = tx.unbounded_send(Action::Toggle);
                }
            }));
            Some(Self { _icon: icon, on_top, autostart, cpu_temp })
        }

        pub fn set(&self, checks: Checks) {
            self.on_top.set_checked(checks.on_top);
            self.autostart.set_checked(checks.autostart);
            if let (Some(item), Some((enabled, _))) = (&self.cpu_temp, checks.cpu_temp) {
                item.set_checked(enabled);
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use ksni::blocking::{Handle, TrayMethods};
    use ksni::menu::{CheckmarkItem, StandardItem};
    use ksni::{Icon, MenuItem};

    use super::*;

    struct Sni {
        tx: UnboundedSender<Action>,
        checks: Checks,
        icon: Icon,
    }

    impl ksni::Tray for Sni {
        fn id(&self) -> String {
            crate::shell::APP_ID.into()
        }

        fn title(&self) -> String {
            "Pulsar".into()
        }

        fn icon_pixmap(&self) -> Vec<Icon> {
            vec![Icon { width: self.icon.width, height: self.icon.height, data: self.icon.data.clone() }]
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            let _ = self.tx.unbounded_send(Action::Toggle);
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            let l = labels::get();
            let send = |action: fn() -> Action| {
                Box::new(move |t: &mut Sni| {
                    let _ = t.tx.unbounded_send(action());
                }) as Box<dyn Fn(&mut Sni) + Send>
            };
            vec![
                StandardItem { label: l.toggle.into(), activate: send(|| Action::Toggle), ..Default::default() }.into(),
                CheckmarkItem {
                    label: l.on_top.into(),
                    checked: self.checks.on_top,
                    activate: send(|| Action::ToggleOnTop),
                    ..Default::default()
                }
                .into(),
                CheckmarkItem {
                    label: l.autostart.into(),
                    checked: self.checks.autostart,
                    activate: send(|| Action::ToggleAutostart),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem { label: l.quit.into(), activate: send(|| Action::Quit), ..Default::default() }.into(),
            ]
        }
    }

    pub struct Tray(Handle<Sni>);

    impl Tray {
        pub fn new(tx: UnboundedSender<Action>, checks: Checks) -> Option<Self> {
            let (rgba, w, h) = icon_rgba();
            // StatusNotifierItem wants ARGB32 in network byte order.
            let data = rgba.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
            let icon = Icon { width: w as i32, height: h as i32, data };
            Sni { tx, checks, icon }.spawn().ok().map(Tray)
        }

        pub fn set(&self, checks: Checks) {
            self.0.update(|t| t.checks = checks);
        }
    }
}

pub use imp::Tray;
