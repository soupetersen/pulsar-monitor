//! A usage ring: a track circle, an accent arc for the value, the value in
//! the middle and a label below.

use gpui::{FontWeight, PathBuilder, Stateful, canvas, div, point, prelude::*, Div};

use super::theme::{Theme, u};

pub fn ring(index: usize, value: f32, label: String, theme: Theme) -> Stateful<Div> {
    let value = value.clamp(0.0, 100.0);
    div()
        .id(("ring", index))
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .gap(u(6.))
        .pt(u(6.))
        .pb(u(8.))
        .rounded(u(10.))
        .hover(move |s| s.bg(theme.hover))
        .child(
            div()
                .relative()
                .size(u(52.))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            // Same geometry as the SVG: r = 18 and stroke 4 in a 44 box.
                            let c = bounds.center();
                            let r = u(52. * 18. / 44.);
                            let w = u(52. * 4. / 44.);
                            let arc = |to: f32| {
                                let steps = (to * 96.0).ceil().max(2.0) as usize;
                                let mut b = PathBuilder::stroke(w);
                                for i in 0..=steps {
                                    let a = (to * i as f32 / steps as f32 * 360.0 - 90.0).to_radians();
                                    let p = point(c.x + r * a.cos(), c.y + r * a.sin());
                                    if i == 0 { b.move_to(p) } else { b.line_to(p) }
                                }
                                b.build().ok()
                            };
                            if let Some(p) = arc(1.0) {
                                window.paint_path(p, theme.track);
                            }
                            if value >= 0.5 {
                                if let Some(p) = arc(value / 100.0) {
                                    window.paint_path(p, theme.accent);
                                }
                            }
                        },
                    )
                    .size_full(),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(u(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{value:.0}%")),
                ),
        )
        .child(div().text_size(u(11.)).text_color(theme.muted).child(label))
}
