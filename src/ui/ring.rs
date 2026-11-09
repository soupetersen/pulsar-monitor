//! A usage ring: a track circle, an accent arc for the value, the value in
//! the middle and a label below. Without a value (no sample yet) it shows a
//! short arc and no number.

use gpui::{Div, FontWeight, PathBuilder, Pixels, Stateful, canvas, div, point, prelude::*};

use super::theme::{Theme, u};

/// The ring alone, `size` wide, with the stroke scaled like the SVG
/// (r = 18 and stroke 4 in a 44 box).
pub fn dial(size: f32, value: Option<f32>, theme: Theme) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let c = bounds.center();
            let r: Pixels = u(size * 18. / 44.);
            let w = u(size * 4. / 44.);
            let arc = |from: f32, to: f32| {
                let steps = ((to - from) * 96.0).ceil().max(2.0) as usize;
                let mut b = PathBuilder::stroke(w);
                for i in 0..=steps {
                    let t = from + (to - from) * i as f32 / steps as f32;
                    let a = (t * 360.0 - 90.0).to_radians();
                    let p = point(c.x + r * a.cos(), c.y + r * a.sin());
                    if i == 0 { b.move_to(p) } else { b.line_to(p) }
                }
                b.build().ok()
            };
            if let Some(p) = arc(0.0, 1.0) {
                window.paint_path(p, theme.track);
            }
            // The loading arc covers 22 of the 113 units of the circumference.
            let to = value.map_or(22.0 / 113.1, |v| v.clamp(0.0, 100.0) / 100.0);
            if to >= 0.005 {
                if let Some(p) = arc(0.0, to) {
                    window.paint_path(p, theme.accent);
                }
            }
        },
    )
    .size(u(size))
}

/// `value` is the number shown and `arc` the (animated) arc, both 0–100.
pub fn ring(index: usize, value: Option<(f32, f32)>, label: String, theme: Theme) -> Stateful<Div> {
    div()
        .id(("ring", index))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(u(6.))
        .pt(u(6.))
        .pb(u(8.))
        .rounded(u(10.))
        .when(value.is_some(), |d| d.hover(move |s| s.bg(theme.hover)))
        .child(
            div().relative().size(u(52.)).child(dial(52., value.map(|v| v.1), theme)).children(value.map(|(v, _)| {
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(u(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{:.0}%", v.clamp(0.0, 100.0).round()))
            })),
        )
        .child(div().text_size(u(11.)).text_color(theme.muted).whitespace_nowrap().child(label))
}
