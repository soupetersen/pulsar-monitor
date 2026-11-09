//! The expanded view: a section per resource with its history and details,
//! the process lists, network and disk, and the version.

use gpui::{Context, Div, FontWeight, MouseButton, SharedString, Stateful, Transformation, div, prelude::*, radians, relative, svg};

use super::assets::icon;
use super::format::{GB, bytes, f1, f2, pct, rate, short_cpu_name, uptime};
use super::compact::{temp_btn, update_btn};
use super::i18n::t;
use super::parts::{alpha, icon_btn, setup_btn, sparkline};
use super::ring::dial;
use super::theme::{Mode, Theme, u};
use super::{Anim, Widget};
use crate::monitor::{Panel, ProcStats, RamStats, Stats};

pub struct History<'a> {
    pub cpu: &'a [f32],
    pub gpu: &'a [f32],
}

fn small(text: impl Into<SharedString>, theme: Theme) -> Div {
    div().text_size(u(11.)).text_color(theme.muted).child(text.into())
}

fn section(theme: Theme) -> Div {
    div().p(u(12.)).rounded(u(12.)).bg(theme.inset).flex().flex_col().gap(u(10.))
}

fn head(left: String, right: Option<String>, theme: Theme) -> Div {
    div()
        .flex()
        .justify_between()
        .gap(u(8.))
        .text_size(u(12.))
        .text_color(theme.muted)
        .child(div().min_w_0().truncate().child(left))
        .children(right.map(|r| div().flex_none().child(r)))
}

fn big(value: String) -> Div {
    div()
        .min_w(u(72.))
        .text_size(u(30.))
        .line_height(u(30.))
        .font_weight(FontWeight::BOLD)
        .child(value)
}

fn big_row(value: String) -> Div {
    div().flex().items_end().gap(u(14.)).child(big(value))
}

/// A label over a value, as in the 4-column stats grid.
fn stat(label: impl Into<SharedString>, value: String, theme: Theme) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(u(2.))
        .child(small(label, theme))
        .child(div().text_size(u(13.)).font_weight(FontWeight::SEMIBOLD).whitespace_nowrap().child(value))
}

/// A label over an element (a setup chip).
fn stat_with(label: &'static str, el: impl IntoElement, theme: Theme) -> Div {
    div().flex_1().min_w_0().flex().flex_col().items_start().gap(u(2.)).child(small(label, theme)).child(el)
}

fn stats_grid(items: Vec<Div>) -> Div {
    div().flex().gap(u(8.)).children(items)
}

fn bar(value: f32, height: f32, theme: Theme) -> Div {
    div()
        .h(u(height))
        .rounded(u(height / 2.))
        .bg(theme.track)
        .child(div().h_full().w(relative(value.clamp(0.0, 100.0) / 100.0)).rounded(u(height / 2.)).bg(theme.accent))
}

fn ram_used_pct(ram: &RamStats) -> f32 {
    (ram.used as f64 / ram.total.max(1) as f64 * 100.0) as f32
}

fn or_dash(v: Option<String>) -> String {
    v.unwrap_or_else(|| "—".into())
}

struct Row {
    name: String,
    sub: String,
    val: String,
    w: f32,
}

fn rows_for(stats: &Stats, kind: Panel) -> Option<Vec<Row>> {
    if stats.procs_for != Some(kind) {
        return None;
    }
    let procs = stats.procs.as_ref()?;
    let total = stats.ram.total.max(1) as f64;
    let metric = |p: &ProcStats| -> f64 {
        match kind {
            Panel::Cpu => p.cpu as f64,
            Panel::Gpu => p.gpu.min(100.0) as f64,
            _ => p.mem as f64,
        }
    };
    let max = procs.iter().map(metric).fold(1e-6, f64::max);
    Some(
        procs
            .iter()
            .map(|p| {
                let many = if p.count > 1 { (t().processes)(p.count) } else { String::new() };
                let (sub, val) = match kind {
                    Panel::Cpu => (vec![many, bytes(p.mem)], format!("{}%", f1(p.cpu as f64))),
                    Panel::Gpu => {
                        let vram = if p.vram > 0 { format!("{} VRAM", bytes(p.vram)) } else { String::new() };
                        (vec![vram, many], pct(p.gpu.min(100.0) as f64))
                    }
                    _ => (vec![(t().of_ram)(&f1(p.mem as f64 / total * 100.0)), many], bytes(p.mem)),
                };
                let sub = sub.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
                Row { name: p.name.clone(), sub, val, w: (metric(p) / max * 100.0).max(3.0) as f32 }
            })
            .collect(),
    )
}

fn proc_list(rows: Option<Vec<Row>>, theme: Theme) -> Div {
    let empty = |text: &'static str| div().px(u(6.)).py(u(8.)).text_size(u(12.)).text_color(theme.muted).child(text);
    match rows {
        None => div().child(empty(t().loading)),
        Some(rows) if rows.is_empty() => div().child(empty(t().no_processes)),
        Some(rows) => div().flex().flex_col().children(rows.into_iter().enumerate().map(|(i, r)| {
            div()
                .id(("proc", i))
                .flex()
                .items_center()
                .gap(u(10.))
                .px(u(6.))
                .py(u(5.))
                .rounded(u(6.))
                .hover(move |s| s.bg(theme.hover))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().text_size(u(12.)).truncate().child(r.name))
                        .when(!r.sub.is_empty(), |d| {
                            d.child(div().text_size(u(11.)).text_color(theme.muted).truncate().child(r.sub))
                        }),
                )
                .child(div().w(u(96.)).flex_none().child(bar(r.w, 4., theme)))
                .child(
                    div()
                        .w(u(58.))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .text_size(u(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(r.val),
                )
        })),
    }
}

/// The "Show … processes" button and, when open, the list.
fn procs(stats: &Stats, anim: Anim, kind: Panel, open: Panel, theme: Theme, cx: &mut Context<Widget>) -> [Option<Div>; 2] {
    let is_open = open == kind;
    let id = match kind {
        Panel::Cpu => "procs-cpu",
        Panel::Gpu => "procs-gpu",
        _ => "procs-ram",
    };
    let toggle: Stateful<Div> = div()
        .id(id)
        .h(u(34.))
        .px(u(10.))
        .rounded(u(8.))
        .bg(theme.chip)
        .hover(move |s| s.bg(theme.hover))
        .flex()
        .items_center()
        .justify_between()
        .text_size(u(12.))
        .font_weight(FontWeight::MEDIUM)
        .child(t().procs_toggle(kind, is_open))
        .child(
            svg()
                .path(icon::CHEVRON)
                .size(u(14.))
                .text_color(theme.text)
                .when(is_open, |s| s.with_transformation(Transformation::rotate(radians(std::f32::consts::PI)))),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_panel(kind, cx)));
    let animated = |mut rows: Vec<Row>| {
        // Bars of processes that were there before slide to the new width.
        let prev = anim.prev.and_then(|p| rows_for(p, kind)).unwrap_or_default();
        for r in &mut rows {
            r.w = anim.mix(prev.iter().find(|p| p.name == r.name).map(|p| p.w), r.w);
        }
        rows
    };
    [Some(div().child(toggle)), is_open.then(|| proc_list(rows_for(stats, kind).map(animated), theme))]
}

fn header(w: &Widget, stats: Option<&Stats>, on_top: bool, theme: Theme, cx: &mut Context<Widget>) -> Div {
    let up = stats.map(|s| (t().uptime)(&uptime(s.uptime_secs))).unwrap_or_else(|| "…".into());
    let (theme_icon, theme_next) = match w.mode {
        Mode::Dark => (icon::SUN, Mode::Light),
        Mode::Light => (icon::MOON, Mode::Dark),
    };
    div()
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(u(1.))
                .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                .child(div().text_size(u(15.)).font_weight(FontWeight::SEMIBOLD).child(t().system))
                .child(small(up, theme)),
        )
        .child(
            div()
                .flex()
                .gap(u(2.))
                .children(update_btn(theme, cx))
                .child(
                    icon_btn("theme", theme_icon, 15., t().change_theme, theme, false)
                        .on_click(cx.listener(move |this, _, window, cx| this.set_mode(theme_next, window, cx))),
                )
                .child(
                    icon_btn("pin", icon::PIN, 14., t().always_on_top, theme, on_top)
                        .on_click(cx.listener(|_, _, _, cx| crate::shell::toggle_on_top(cx))),
                )
                .child(
                    icon_btn("collapse", icon::COLLAPSE, 14., t().collapse, theme, false)
                        .on_click(cx.listener(|this, _, _, cx| this.collapse(cx))),
                )
                .child(
                    icon_btn("hide", icon::MINIMIZE, 14., t().minimize_to_tray, theme, false)
                        .on_click(cx.listener(|_, _, _, cx| crate::shell::hide(cx))),
                ),
        )
}

pub fn render(
    w: &Widget,
    stats: Option<&Stats>,
    anim: Anim,
    hist: History,
    on_top: bool,
    theme: Theme,
    cx: &mut Context<Widget>,
) -> Div {
    let card = div()
        .flex()
        .flex_col()
        .gap(u(10.))
        .pt(u(12.))
        .px(u(14.))
        .pb(u(14.))
        .child(header(w, stats, on_top, theme, cx));

    let Some(s) = stats else {
        return card.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(u(10.))
                .py(u(48.))
                .child(dial(28., None, theme))
                .child(div().text_size(u(12.)).text_color(theme.muted).child(t().collecting)),
        );
    };
    let panel = w.panel;
    let sensors = crate::shell::sensors(cx);

    // CPU
    let c = &s.cpu;
    let cpu = section(theme)
        .child(head(format!("CPU · {}", short_cpu_name(&c.name)), None, theme))
        .child(big_row(pct(c.usage as f64)).child(sparkline(hist.cpu.to_vec(), theme)))
        .when(!c.cores.is_empty(), |d| {
            let prev = anim.prev.map(|p| &p.cpu.cores).filter(|p| p.len() == c.cores.len());
            d.child(div().flex().items_end().gap(u(3.)).h(u(26.)).children(c.cores.iter().enumerate().map(|(i, core)| {
                let core = anim.mix(prev.map(|p| p[i]), *core);
                div()
                    .flex_1()
                    .h(relative(core.clamp(8.0, 100.0) / 100.0))
                    .rounded(u(2.))
                    .bg(alpha(theme.accent, 0.7))
            })))
        })
        .child(stats_grid(vec![
            match (c.temp_c, sensors.temp_setup) {
                (None, Some(pawnio)) => stat_with(t().temp, temp_btn(pawnio, sensors.temp_busy, true, theme, cx), theme),
                (temp, _) => stat(t().temp, or_dash(temp.map(|v| format!("{}°C", v.round()))), theme),
            },
            match (c.power_w, sensors.power_setup) {
                (None, true) => {
                    let busy = sensors.power_busy;
                    let tip = if busy { t().cpu_power_enabling } else { t().cpu_power_off };
                    let chip = setup_btn("power-setup", icon::BOLT, tip, busy, true, theme)
                        .when(!busy, |d| d.on_click(cx.listener(|_, _, _, cx| crate::shell::enable_power(cx))));
                    stat_with(t().power, chip, theme)
                }
                (power, _) => stat(t().power, or_dash(power.map(|v| format!("{} W", v.round()))), theme),
            },
            stat("Clock", or_dash(c.clock_ghz.filter(|g| *g > 0.0).map(|g| format!("{} GHz", f2(g as f64)))), theme),
            stat("Threads", c.threads.to_string(), theme),
        ]))
        .children(procs(s, anim, Panel::Cpu, panel, theme, cx).into_iter().flatten());

    // GPU
    let gpu = s.gpu.as_ref().map(|g| {
        let vram = match (g.vram_used, g.vram_total.filter(|t| *t > 0)) {
            (Some(used), Some(total)) => {
                let total = total as f64;
                let total = if total >= 10.0 * GB { format!("{}", (total / GB).round()) } else { f1(total / GB) };
                Some(format!("{} / {total} GB", f1(used as f64 / GB)))
            }
            _ => None,
        };
        let power = g.power_w.map(|w| format!("{} W", w.round())).or(g.power_pct.map(|p| format!("{}%", p.round())));
        let fan = g.fan_pct.map(|p| format!("{p}%")).or(g.fan_rpm.map(|r| format!("{r} rpm")));
        section(theme)
            .child(head(format!("GPU · {}", g.name), g.clock_mhz.map(|m| format!("{m} MHz")), theme))
            .child(big_row(pct(g.usage as f64)).child(sparkline(hist.gpu.to_vec(), theme)))
            .child(stats_grid(vec![
                stat(t().temp, or_dash(g.temp_c.map(|t| format!("{t}°C"))), theme),
                stat(t().power, or_dash(power), theme),
                stat("VRAM", or_dash(vram), theme),
                stat(t().fan, or_dash(fan), theme),
            ]))
            .children(procs(s, anim, Panel::Gpu, panel, theme, cx).into_iter().flatten())
    });

    // Memory
    let ram = &s.ram;
    let ram_pct = ram_used_pct(ram);
    let memory = section(theme)
        .child(head(
            t().memory.into(),
            Some(format!("{} / {} GB", f1(ram.used as f64 / GB), f1(ram.total as f64 / GB))),
            theme,
        ))
        .child(
            big_row(pct(ram_pct as f64)).child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(u(6.))
                    .child(bar(anim.mix(anim.prev.map(|p| ram_used_pct(&p.ram)), ram_pct), 8., theme))
                    .child(small((t().available)(&f1(ram.available as f64 / GB)), theme)),
            ),
        )
        .children(procs(s, anim, Panel::Ram, panel, theme, cx).into_iter().flatten());

    // Network and disk
    let mini = |label: &'static str, value: String| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(u(2.))
            .py(u(10.))
            .px(u(12.))
            .rounded(u(12.))
            .bg(theme.inset)
            .child(small(label, theme))
            .child(div().text_size(u(13.)).font_weight(FontWeight::SEMIBOLD).whitespace_nowrap().child(value))
    };
    let net = or_dash(s.net.as_ref().map(|n| format!("↓ {} · ↑ {}", rate(n.rx_per_sec), rate(n.tx_per_sec))));
    let disk = or_dash(s.disk.as_ref().map(|d| format!("{} · {}", pct(d.active_pct), rate(d.bytes_per_sec))));
    let pair = div().flex().gap(u(10.)).child(mini(t().network, net)).child(mini(t().disk, disk));

    card.child(cpu)
        .children(gpu)
        .child(memory)
        .child(pair)
        .child(
            div()
                .mt(u(-2.))
                .flex()
                .justify_center()
                .text_size(u(11.))
                .text_color(theme.muted)
                .child(concat!("Pulsar ", env!("CARGO_PKG_VERSION"))),
        )
}
