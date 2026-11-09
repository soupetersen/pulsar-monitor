//! Everything around the views: the window and its native behaviour, and
//! the sampler thread feeding it.

mod window;

use std::sync::{Arc, Mutex};

use gpui::{App, Application, Bounds, WindowBounds, WindowKind, WindowOptions, prelude::*, size};

use crate::monitor::{self, Panel, SharedView, View};
use crate::ui::{Widget, theme::{Mode, u}};

pub const APP_ID: &str = "com.petersen.pulsar";

pub fn run() {
    let view: SharedView = Arc::new(Mutex::new(View { visible: true, expanded: false, panel: Panel::None }));
    let (tx, rx) = futures::channel::mpsc::unbounded();
    monitor::spawn(view.clone(), move |stats| {
        let _ = tx.unbounded_send(stats);
    });

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(u(380.), u(128.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            // No taskbar entry; always on top is set natively (see window.rs).
            kind: WindowKind::PopUp,
            window_background: window::background(),
            is_resizable: false,
            is_minimizable: false,
            focus: true,
            show: true,
            app_id: Some(APP_ID.into()),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            window::apply_native_style(window);
            cx.new(|cx| Widget::new(rx, Mode::Dark, cx))
        })
        .expect("failed to open the window");
    });
}
