//! UI strings. English by default, Portuguese when the system language is
//! Portuguese.

use std::sync::OnceLock;

use crate::monitor::Panel;

pub struct Strings {
    pub expand: &'static str,
    pub minimize_to_tray: &'static str,
    pub system: &'static str,
    pub uptime: fn(&str) -> String,
    pub change_theme: &'static str,
    pub always_on_top: &'static str,
    pub collapse: &'static str,
    show_procs: [&'static str; 3],
    hide_procs: [&'static str; 3],
    pub temp: &'static str,
    pub power: &'static str,
    pub fan: &'static str,
    pub memory: &'static str,
    pub available: fn(&str) -> String,
    pub network: &'static str,
    pub disk: &'static str,
    pub collecting: &'static str,
    pub loading: &'static str,
    pub no_processes: &'static str,
    pub processes: fn(u32) -> String,
    pub of_ram: fn(&str) -> String,
}

fn index(panel: Panel) -> usize {
    match panel {
        Panel::Gpu => 1,
        Panel::Ram => 2,
        _ => 0,
    }
}

impl Strings {
    pub fn procs_toggle(&self, panel: Panel, open: bool) -> &'static str {
        if open { self.hide_procs[index(panel)] } else { self.show_procs[index(panel)] }
    }
}

const EN: Strings = Strings {
    expand: "Expand",
    minimize_to_tray: "Minimize to tray",
    system: "System",
    uptime: |t| format!("Up for {t}"),
    change_theme: "Change theme",
    always_on_top: "Always on top",
    collapse: "Collapse",
    show_procs: ["Show CPU processes", "Show GPU processes", "Show RAM processes"],
    hide_procs: ["Hide CPU processes", "Hide GPU processes", "Hide RAM processes"],
    temp: "Temp.",
    power: "Power",
    fan: "Fan",
    memory: "Memory",
    available: |gb| format!("{gb} GB available"),
    network: "Network",
    disk: "Disk",
    collecting: "Collecting data…",
    loading: "Loading…",
    no_processes: "No processes using it right now.",
    processes: |n| format!("{n} processes"),
    of_ram: |pct| format!("{pct}% of RAM"),
};

const PT: Strings = Strings {
    expand: "Expandir",
    minimize_to_tray: "Minimizar para a bandeja",
    system: "Sistema",
    uptime: |t| format!("Ligado há {t}"),
    change_theme: "Trocar tema",
    always_on_top: "Sempre no topo",
    collapse: "Recolher",
    show_procs: ["Ver processos na CPU", "Ver processos na GPU", "Ver processos na RAM"],
    hide_procs: ["Ocultar processos na CPU", "Ocultar processos na GPU", "Ocultar processos na RAM"],
    temp: "Temp.",
    power: "Potência",
    fan: "Ventoinha",
    memory: "Memória",
    available: |gb| format!("Disponível {gb} GB"),
    network: "Rede",
    disk: "Disco",
    collecting: "Coletando dados…",
    loading: "Carregando…",
    no_processes: "Nenhum processo usando agora.",
    processes: |n| format!("{n} processos"),
    of_ram: |pct| format!("{pct}% da RAM"),
};

fn locale() -> &'static str {
    static LOCALE: OnceLock<String> = OnceLock::new();
    LOCALE.get_or_init(|| sys_locale::get_locale().unwrap_or_else(|| "en-US".into()).to_lowercase())
}

pub fn t() -> &'static Strings {
    if locale().starts_with("pt") { &PT } else { &EN }
}

/// Whether the system language writes decimals with a comma (1,5 GB).
pub fn decimal_comma() -> bool {
    const POINT: &[&str] = &["en", "ja", "zh", "ko", "he", "th", "hi", "ms", "fil", "ga"];
    let lang = locale().split(['-', '_']).next().unwrap_or("en");
    !POINT.contains(&lang)
}
