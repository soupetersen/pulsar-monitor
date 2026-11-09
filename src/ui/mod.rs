//! gpui views. `Widget` is the window's root: it redraws on every sample
//! from the sampler and shows the compact or the expanded view. The window
//! always hugs the content, so it grows and shrinks with the view.

mod assets;
mod compact;
mod expanded;
mod format;
mod i18n;
mod parts;
mod ring;
pub mod theme;

use std::cell::Cell;
use std::rc::Rc;

use gpui::{Context, Entity, Pixels, Render, Size, Window, canvas, div, prelude::*, px, size};

pub use assets::Assets;
use crate::monitor::{Panel, Stats};
use crate::shell::{self, StatsStore};
use theme::{Mode, Theme, font_family, u};

/// The animation from the previous sample to the latest: bars and rings
/// show `prev` mixed into the new value by `p` (0 to 1).
#[derive(Clone, Copy)]
pub struct Anim<'a> {
    pub prev: Option<&'a Stats>,
    pub p: f32,
}

impl Anim<'_> {
    pub fn mix(&self, from: Option<f32>, to: f32) -> f32 {
        match from {
            Some(from) if self.p < 1.0 => from + (to - from) * self.p,
            _ => to,
        }
    }
}

pub struct Widget {
    stats: Entity<StatsStore>,
    mode: Mode,
    expanded: bool,
    /// The open process list. Kept while collapsed, so expanding again
    /// shows the same list.
    panel: Panel,
    fitted: Rc<Cell<(Size<Pixels>, f32)>>,
}

impl Widget {
    pub fn new(stats: Entity<StatsStore>, mode: Mode, cx: &mut Context<Self>) -> Self {
        cx.observe(&stats, |_, _, cx| cx.notify()).detach();
        let (expanded, panel) = shell::view_state(cx);
        Self { stats, mode, expanded, panel, fitted: Rc::default() }
    }

    fn expand(&mut self, panel: Option<Panel>, cx: &mut Context<Self>) {
        if let Some(panel) = panel {
            self.panel = panel;
        }
        self.expanded = true;
        self.sync(cx);
    }

    fn collapse(&mut self, cx: &mut Context<Self>) {
        self.expanded = false;
        self.sync(cx);
    }

    fn toggle_panel(&mut self, panel: Panel, cx: &mut Context<Self>) {
        self.panel = if self.panel == panel { Panel::None } else { panel };
        self.sync(cx);
    }

    fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        shell::set_theme(mode, window, cx);
        cx.notify();
    }

    /// Tells the sampler what is on screen, so it only collects that.
    fn sync(&mut self, cx: &mut Context<Self>) {
        shell::set_view(self.expanded, self.panel, cx);
        cx.notify();
    }
}

/// Resizes the window to the bounds of the element it is put in, whenever
/// they or the window's scale (moved to another monitor) change. `last`
/// holds what the last resize was for: the window itself may end up a bit
/// off (rounding) or smaller (capped to the screen, where the view scrolls).
fn fit_window(last: Rc<Cell<(Size<Pixels>, f32)>>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            let ceil = |p: Pixels| px(f32::from(p).ceil());
            let want = (size(ceil(bounds.size.width), ceil(bounds.size.height)), window.scale_factor());
            if last.get() != want {
                last.set(want);
                window.defer(cx, move |window, cx| shell::fit_window(want.0, window, cx));
            }
        },
    )
    .absolute()
    .inset_0()
}

impl Render for Widget {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(self.mode);
        let store = self.stats.read(cx);
        let stats = store.latest.clone();
        let prev = store.prev.clone();
        let p = store.progress();
        let cpu_hist = store.cpu_hist.clone();
        let gpu_hist = store.gpu_hist.clone();
        let on_top = shell::on_top(cx);

        let anim = Anim { prev: prev.as_ref(), p };
        let (content, width) = if self.expanded {
            let hist = expanded::History { cpu: &cpu_hist, gpu: &gpu_hist };
            (expanded::render(self, stats.as_ref(), anim, hist, on_top, theme, cx), 440.)
        } else {
            (compact::render(stats.as_ref(), anim, theme, cx), 372.)
        };

        div()
            .id("root")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.card)
            // Windows rounds the window itself; elsewhere the transparent
            // window needs the card rounded.
            .when(!cfg!(windows), |d| d.rounded(u(12.)))
            .text_color(theme.text)
            .font_family(font_family())
            .child(content.relative().flex_none().w(u(width)).child(fit_window(self.fitted.clone())))
    }
}
