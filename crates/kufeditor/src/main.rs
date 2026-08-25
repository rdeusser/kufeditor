mod actions;
mod catalog_status;
mod components;
mod frame;
mod notices;
mod number_edit;
mod settings;
mod state;
mod text_input;
mod theme;
mod views;

use std::{cell::Cell, process::ExitCode, rc::Rc};

use gpui::{
    AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

use crate::frame::AppFrame;
use crate::settings::{SettingsStartup, settings_path};

fn main() -> ExitCode {
    let startup_failed = Rc::new(Cell::new(false));
    let failure_in_app = Rc::clone(&startup_failed);
    let startup = SettingsStartup::load(settings_path());

    Application::new().run(move |cx| {
        actions::bind(cx);
        text_input::bind(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
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
                let frame = cx.new(|cx| AppFrame::new(startup, cx));
                frame.update(cx, AppFrame::start_catalog_load);
                let weak_frame = frame.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    weak_frame
                        .update(cx, |frame, frame_cx| {
                            frame.window_should_close(window, frame_cx)
                        })
                        .unwrap_or(true)
                });
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
