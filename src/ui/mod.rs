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
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    Bounds, Context, Entity, EntityInputHandler, FocusHandle, KeyDownEvent, Pixels, Point, Render, ScrollStrategy, Size,
    UTF16Selection, UniformListScrollHandle, Window, canvas, div, prelude::*, px, size,
};

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
    /// What the process lists are filtered by, and the search field's focus.
    query: String,
    search: FocusHandle,
    proc_scroll: UniformListScrollHandle,
    fitted: Rc<Cell<(Size<Pixels>, f32)>>,
}

impl Widget {
    pub fn new(stats: Entity<StatsStore>, mode: Mode, cx: &mut Context<Self>) -> Self {
        cx.observe(&stats, |_, _, cx| cx.notify()).detach();
        let (expanded, panel) = shell::view_state(cx);
        Self {
            stats,
            mode,
            expanded,
            panel,
            query: String::new(),
            search: cx.focus_handle(),
            proc_scroll: UniformListScrollHandle::new(),
            fitted: Rc::default(),
        }
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
        self.query.clear();
        self.proc_scroll.scroll_to_item(0, ScrollStrategy::Top);
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

    /// Keys in the process search field. Typed text comes through the
    /// input handler below, which also gets accents and IME input.
    fn type_query(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let m = &key.modifiers;
        // AltGr arrives as Ctrl+Alt on Windows and types characters.
        let shortcut = m.platform || (m.control && !m.alt);
        match key.key.as_str() {
            "escape" if self.query.is_empty() => window.blur(),
            "escape" => self.query.clear(),
            "backspace" if shortcut => self.query.clear(),
            "backspace" => _ = self.query.pop(),
            "v" if shortcut => {
                let text = cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default();
                return self.add_to_query(text.lines().next().unwrap_or("").trim(), cx);
            }
            _ => return,
        }
        self.query_changed(cx);
    }

    fn add_to_query(&mut self, text: &str, cx: &mut Context<Self>) {
        self.query.extend(text.chars().filter(|c| !c.is_control()));
        self.query = self.query.chars().take(48).collect();
        self.query_changed(cx);
    }

    fn query_changed(&mut self, cx: &mut Context<Self>) {
        self.proc_scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.stop_propagation();
        cx.notify();
    }
}

/// The search field takes text at its end only: no selection, no caret
/// moves, no text being composed.
impl EntityInputHandler for Widget {
    fn text_for_range(&mut self, _: Range<usize>, _: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let end = self.query.encode_utf16().count();
        Some(UTF16Selection { range: end..end, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.add_to_query(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }

    fn bounds_for_range(&mut self, _: Range<usize>, bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            let searching = self.search.is_focused(window);
            (expanded::render(self, stats.as_ref(), anim, hist, on_top, searching, theme, cx), 440.)
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
