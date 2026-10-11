#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod i18n;
mod single_instance;
mod wslc;

use gpui_kit::component::{Theme, ThemeMode, TitleBar};
use gpui_kit::*;

use app::AppView;

actions!(wslc_desktop, [Quit]);

fn main() {
    // Keep the mutex alive for the process lifetime (dropped after `app.run`).
    let _instance = match single_instance::try_acquire() {
        Ok(lock) => Some(lock),
        Err(single_instance::AcquireError::AlreadyRunning) => {
            single_instance::notify_already_running();
            return;
        }
        Err(single_instance::AcquireError::Failed(_)) => {
            // Mutex creation failed; continue rather than blocking launch.
            None
        }
    };

    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    app.run(move |cx| {
        gpui_kit::init(cx);
        // Override init's default Light theme before the first frame is shown.
        Theme::change(ThemeMode::Dark, None, cx);

        cx.bind_keys([KeyBinding::new("alt-f4", Quit, None)]);

        cx.on_action(|_: &Quit, cx: &mut App| {
            cx.quit();
        });

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1080.), px(720.)), cx)),
            ..TitleBar::window_options()
        };

        gpui_kit::open_window(window_options, cx, |window, cx| {
            window.activate_window();
            window.set_window_title("WSLc Desktop");
            cx.new(|cx| AppView::new(window, cx))
        })
        .expect("Failed to open window");
    });
}
