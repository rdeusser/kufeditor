mod actions;
mod components;
mod frame;
mod number_edit;
mod state;
mod theme;
mod views;

use std::{cell::Cell, process::ExitCode, rc::Rc};

use gpui::{
    AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

use crate::frame::AppFrame;

fn main() -> ExitCode {
    let startup_failed = Rc::new(Cell::new(false));
    let failure_in_app = Rc::clone(&startup_failed);

    Application::new().run(move |cx| {
        actions::bind(cx);
        let bounds = Bounds::centered(None, size(px(1180.0), px(780.0)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("KufEditor".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let frame = cx.new(AppFrame::new);
                window.focus(&frame.read(cx).focus_handle());
                frame
            },
        );

        if let Err(error) = opened {
            failure_in_app.set(true);
            eprintln!("kufeditor: {error}");
            cx.quit();
            return;
        }

        cx.activate(true);
    });

    if startup_failed.get() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
