mod actions;
mod catalog_status;
mod components;
mod crusaders_catalog_status;
mod float_edit;
mod frame;
mod mod_status;
mod notices;
mod number_edit;
mod patch_status;
mod settings;
mod state;
#[cfg(test)]
mod test_support;
mod text_input;
mod theme;
mod views;

use std::{cell::Cell, process::ExitCode, rc::Rc};

use gpui::{
    AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

use crate::frame::AppFrame;
use crate::settings::{SettingsStartup, settings_path};

const WINDOW_WIDTH: f32 = 1320.0;
const WINDOW_HEIGHT: f32 = 840.0;
const WINDOW_MIN_WIDTH: f32 = 1180.0;
const WINDOW_MIN_HEIGHT: f32 = 720.0;

fn minimum_window_size() -> gpui::Size<gpui::Pixels> {
    size(px(WINDOW_MIN_WIDTH), px(WINDOW_MIN_HEIGHT))
}

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
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(minimum_window_size()),
                titlebar: Some(TitlebarOptions {
                    title: Some("KufEditor".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let frame = cx.new(|cx| AppFrame::new(startup, cx));
                frame.update(cx, AppFrame::start_catalog_load);
                frame.update(cx, AppFrame::reconcile_crusaders_catalog);
                frame.update(cx, AppFrame::start_mod_library_scan);
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

#[cfg(test)]
mod tests {
    use gpui::{px, size};

    use super::minimum_window_size;

    #[test]
    fn window_minimum_preserves_the_focused_split_shell() {
        assert_eq!(minimum_window_size(), size(px(1180.0), px(720.0)));
    }
}
