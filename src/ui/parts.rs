//! Small pieces shared by both views: icon buttons and the sparkline.

use gpui::{Context, Div, FontWeight, PathBuilder, Render, Rgba, Stateful, Window, canvas, div, point, prelude::*, px, svg};

use super::theme::{Theme, font_family, u};

/// A hover tooltip.
pub struct Tip {
    text: &'static str,
    theme: Theme,
}

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .font_family(font_family())
            .text_size(u(12.))
            .text_color(theme.text)
            .bg(theme.tip)
            .border_1()
            .border_color(theme.track)
            .rounded(u(6.))
            .px(u(8.))
            .py(u(4.))
            .child(self.text)
    }
}

/// A 30×30 header button with an icon, like `.icon-btn` in app.css.
pub fn icon_btn(
    id: &'static str,
    icon: &'static str,
    icon_size: f32,
    tip: &'static str,
    theme: Theme,
    active: bool,
) -> Stateful<Div> {
    let (color, hover_color) = if active { (theme.accent, theme.accent) } else { (theme.muted, theme.text) };
    div()
        .id(id)
        .group(id)
        .size(u(30.))
        .flex_none()
        .rounded(u(6.))
        .flex()
        .items_center()
        .justify_center()
        .when(active, |d| d.bg(theme.chip))
        .hover(move |s| s.bg(theme.hover))
        .tooltip(move |_, cx| cx.new(|_| Tip { text: tip, theme }).into())
        .child(
            svg()
                .path(icon)
                .size(u(icon_size))
                .text_color(color)
                .group_hover(id, move |s| s.text_color(hover_color)),
        )
}

/// A CPU sensor that needs a one-time setup (temperature, or power on
/// Linux): an icon with a dot, plus "Enable" as the chip in the stats grid
/// (`label`). Like TempSetup.svelte.
pub fn setup_btn(
    id: &'static str,
    icon: &'static str,
    tip: &'static str,
    busy: bool,
    label: bool,
    theme: Theme,
) -> Stateful<Div> {
    let t = super::i18n::t();
    let color = if label { theme.text } else { theme.muted };
    div()
        .id(id)
        .group(id)
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(u(6.))
        .rounded(u(6.))
        .text_color(color)
        .map(|d| {
            if label {
                d.h(u(24.)).pl(u(6.)).pr(u(8.)).bg(theme.chip).text_size(u(12.)).font_weight(FontWeight::SEMIBOLD)
            } else {
                d.h(u(30.)).min_w(u(30.)).px(u(8.))
            }
        })
        .when(!busy, |d| d.hover(move |s| s.bg(theme.hover)))
        .tooltip(move |_, cx| cx.new(|_| Tip { text: tip, theme }).into())
        .child(
            svg()
                .path(icon)
                .size(u(14.))
                .text_color(if busy { alpha(color, 0.45) } else { color })
                .when(!label, |s| s.group_hover(id, move |s| s.text_color(theme.text))),
        )
        .child(
            div()
                .absolute()
                .top(u(if label { 3. } else { 5. }))
                .left(u(if label { 15. } else { 17. }))
                .size(u(6.))
                .rounded_full()
                .bg(theme.accent),
        )
        .when(label, |d| d.child(if busy { t.enabling } else { t.enable }))
}

pub fn alpha(color: Rgba, a: f32) -> Rgba {
    Rgba { a, ..color }
}

/// The usage history (0–100, oldest first) as a filled line. It is drawn
/// right-aligned over `SLOTS` samples, so it grows from the right.
pub fn sparkline(values: Vec<f32>, theme: Theme) -> impl IntoElement {
    const SLOTS: f32 = 30.0;
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let n = values.len();
            if n < 2 {
                return;
            }
            let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            // Same mapping as the SVG: 2 px from the bottom, 4 px from the top.
            let k = h / 36.0;
            let points: Vec<_> = values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let x = x0 + w - (n - 1 - i) as f32 * w / (SLOTS - 1.0);
                    let y = y0 + h - 2.0 * k - v.clamp(0.0, 100.0) / 100.0 * (h - 6.0 * k);
                    point(px(x), px(y))
                })
                .collect();

            let mut area = PathBuilder::fill();
            area.move_to(point(points[0].x, px(y0 + h)));
            for p in &points {
                area.line_to(*p);
            }
            area.line_to(point(points[n - 1].x, px(y0 + h)));
            area.close();
            if let Ok(path) = area.build() {
                window.paint_path(path, alpha(theme.accent, 0.12));
            }

            let mut line = PathBuilder::stroke(u(2.));
            line.move_to(points[0]);
            for p in &points[1..] {
                line.line_to(*p);
            }
            if let Ok(path) = line.build() {
                window.paint_path(path, theme.accent);
            }
        },
    )
    .flex_1()
    .min_w_0()
    .h(u(36.))
}
