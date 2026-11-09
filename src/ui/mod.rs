//! gpui views. `Widget` is the window's root: it receives a `Stats` from the
//! sampler every second and draws the compact view.

mod ring;
pub mod theme;

use futures::StreamExt;
use futures::channel::mpsc::UnboundedReceiver;
use gpui::{Context, FontWeight, MouseButton, Render, Window, div, prelude::*};

use crate::monitor::Stats;
use theme::{Mode, Theme, font_family, u};

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

pub struct Widget {
    stats: Option<Stats>,
    mode: Mode,
}

impl Widget {
    pub fn new(mut rx: UnboundedReceiver<Stats>, mode: Mode, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            while let Some(stats) = rx.next().await {
                let alive = this.update(cx, |view, cx| {
                    view.stats = Some(stats);
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        Self { stats: None, mode }
    }
}

struct RingData {
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
    let mut list = vec![RingData { value: cpu.usage, label: cpu_label }];
    if let Some(gpu) = &stats.gpu {
        let label = gpu.temp_c.map(|t| format!("GPU · {t}°C")).unwrap_or_else(|| "GPU".into());
        list.push(RingData { value: gpu.usage, label });
    }
    let ram = &stats.ram;
    list.push(RingData {
        value: (ram.used as f64 / ram.total.max(1) as f64 * 100.0) as f32,
        label: format!("RAM · {:.1} GB", ram.used as f64 / GB),
    });
    if let Some(gpu) = &stats.gpu {
        if let (Some(used), Some(total)) = (gpu.vram_used, gpu.vram_total.filter(|t| *t > 0)) {
            list.push(RingData {
                value: (used as f64 / total as f64 * 100.0) as f32,
                label: format!("VRAM · {:.1} GB", used as f64 / GB),
            });
        }
    }
    list
}

impl Render for Widget {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(self.mode);
        let host = self.stats.as_ref().map(|s| s.host.clone()).unwrap_or_else(|| "…".into());
        let rings = self.stats.as_ref().map(rings).unwrap_or_default();

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(u(4.))
            .pt(u(8.))
            .px(u(10.))
            .pb(u(10.))
            .bg(theme.card)
            .text_color(theme.text)
            .font_family(font_family())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl(u(2.))
                    .h(u(30.))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(u(8.))
                            .text_size(u(12.))
                            .text_color(theme.muted)
                            .child(div().size(u(7.)).rounded_full().bg(theme.accent))
                            .child(host),
                    ),
            )
            .child(
                div().flex().gap(u(4.)).children(
                    rings
                        .into_iter()
                        .enumerate()
                        .map(|(i, r)| ring::ring(i, r.value, r.label, theme).font_weight(FontWeight::NORMAL)),
                ),
            )
    }
}
