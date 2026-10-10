mod app;
mod wslc;

use gpui_kit::component::{TitleBar, Theme, ThemeMode};
use gpui_kit::*;

use app::AppView;

actions!(wslc_desktop, [Quit]);

fn main() {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    app.run(move |cx| {
        gpui_kit::init(cx);

        cx.bind_keys([
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-q", Quit, None),
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("alt-f4", Quit, None),
        ]);

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
            Theme::change(ThemeMode::Dark, Some(window), cx);
            cx.new(|cx| AppView::new(window, cx))
        })
        .expect("Failed to open window");
    });
}
