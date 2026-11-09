//! Tray menu labels: English by default, Portuguese when the system
//! language is Portuguese.

pub struct Labels {
    pub toggle: &'static str,
    pub on_top: &'static str,
    pub autostart: &'static str,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub cpu_temp: &'static str,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub cpu_temp_needs_pawnio: &'static str,
    pub quit: &'static str,
}

pub fn get() -> Labels {
    let pt = sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("pt"));
    let windows = cfg!(windows);
    if pt {
        Labels {
            toggle: "Mostrar / ocultar",
            on_top: "Sempre no topo",
            autostart: if windows { "Iniciar com o Windows" } else { "Iniciar com o sistema" },
            cpu_temp: "Temperatura da CPU (requer admin)",
            cpu_temp_needs_pawnio: "Temperatura da CPU (requer PawnIO)",
            quit: "Sair",
        }
    } else {
        Labels {
            toggle: "Show / hide",
            on_top: "Always on top",
            autostart: if windows { "Start with Windows" } else { "Start at login" },
            cpu_temp: "CPU temperature (needs admin)",
            cpu_temp_needs_pawnio: "CPU temperature (needs PawnIO)",
            quit: "Quit",
        }
    }
}
