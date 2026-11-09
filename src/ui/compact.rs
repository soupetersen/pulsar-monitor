//! The compact view: the host name, the header buttons and one ring per
//! resource. Clicking a ring opens the expanded view on its processes.

use gpui::{Context, Div, MouseButton, Stateful, div, prelude::*};

use super::assets::icon;
use super::format::{GB, f1};
use super::parts::{icon_btn, setup_btn};
use super::ring::ring;
use super::theme::{Theme, u};
use super::{Anim, Widget, i18n::t};
use crate::monitor::{Panel, Stats};
use crate::shell;

struct RingData {
    panel: Panel,
    value: f32,
    label: String,
}

fn rings(stats: &Stats) -> Vec<RingData> {
    let cpu = &stats.cpu;
    let cpu_label = match (cpu.temp_c, cpu.power_w) {
        (Some(t), _) => format!("CPU · {}°C", t.round()),
        (None, Some(w)) => format!("CPU · {} W", w.round()),
        _ => "CPU".into(),
    };
    let mut list = vec![RingData { panel: Panel::Cpu, value: cpu.usage, label: cpu_label }];
    if let Some(gpu) = &stats.gpu {
        let label = gpu.temp_c.map(|t| format!("GPU · {t}°C")).unwrap_or_else(|| "GPU".into());
        list.push(RingData { panel: Panel::Gpu, value: gpu.usage, label });
    }
    let ram = &stats.ram;
    list.push(RingData {
        panel: Panel::Ram,
        value: (ram.used as f64 / ram.total.max(1) as f64 * 100.0) as f32,
        label: format!("RAM · {} GB", f1(ram.used as f64 / GB)),
    });
    if let Some(gpu) = &stats.gpu {
        if let (Some(used), Some(total)) = (gpu.vram_used, gpu.vram_total.filter(|t| *t > 0)) {
            list.push(RingData {
                panel: Panel::Gpu,
                value: (used as f64 / total as f64 * 100.0) as f32,
                label: format!("VRAM · {} GB", f1(used as f64 / GB)),
            });
        }
    }
    list
}

/// The CPU temperature setup button, while it can be turned on.
fn temp_setup(theme: Theme, cx: &mut Context<Widget>) -> Option<Stateful<Div>> {
    let sensors = shell::sensors(cx);
    let pawnio = sensors.temp_setup?;
    Some(temp_btn(pawnio, sensors.temp_busy, false, theme, cx))
}

pub fn temp_btn(pawnio: bool, busy: bool, label: bool, theme: Theme, cx: &mut Context<Widget>) -> Stateful<Div> {
    let tip = if busy {
        t().cpu_temp_enabling
    } else if pawnio {
        t().cpu_temp_off
    } else {
        t().cpu_temp_needs_pawnio
    };
    let id = if label { "temp-setup-chip" } else { "temp-setup" };
    setup_btn(id, icon::THERMOMETER, tip, busy, label, theme)
        .when(!busy, |d| d.on_click(cx.listener(|_, _, _, cx| shell::enable_cpu_temp(cx))))
}

pub fn render(stats: Option<&Stats>, anim: Anim, theme: Theme, cx: &mut Context<Widget>) -> Div {
    let prev = anim.prev.map(rings);
    let host = stats.map(|s| s.host.clone()).unwrap_or_else(|| "…".into());

    let header = div()
        .flex()
        .items_center()
        .h(u(30.))
        .child(
            // The drag area: everything in the header but the buttons.
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .gap(u(8.))
                .pl(u(2.))
                .text_size(u(12.))
                .text_color(theme.muted)
                .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                .child(div().size(u(7.)).rounded_full().bg(theme.accent))
                .child(host),
        )
        .child(
            div()
                .flex()
                .gap(u(2.))
                .children(temp_setup(theme, cx))
                .child(
                    icon_btn("expand", icon::EXPAND, 14., t().expand, theme, false)
                        .on_click(cx.listener(|this, _, _, cx| this.expand(None, cx))),
                )
                .child(
                    icon_btn("hide", icon::MINIMIZE, 14., t().minimize_to_tray, theme, false)
                        .on_click(cx.listener(|_, _, _, cx| crate::shell::hide(cx))),
                ),
        );

    let rings = match stats {
        Some(stats) => rings(stats)
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                let panel = r.panel;
                let arc = anim.mix(prev.as_ref().and_then(|p| p.get(i)).map(|p| p.value), r.value);
                ring(i, Some((r.value, arc)), r.label, theme)
                    .on_click(cx.listener(move |this, _, _, cx| this.expand(Some(panel), cx)))
            })
            .collect::<Vec<_>>(),
        None => ["CPU", "GPU", "RAM", "VRAM"]
            .into_iter()
            .enumerate()
            .map(|(i, label)| ring(i, None, label.into(), theme))
            .collect(),
    };

    div()
        .flex()
        .flex_col()
        .gap(u(4.))
        .pt(u(8.))
        .px(u(10.))
        .pb(u(10.))
        .child(header)
        .child(div().flex().gap(u(4.)).children(rings))
}

