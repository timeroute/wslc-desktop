#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod i18n;
mod single_instance;
mod wslc;

use gpui_kit::component::{Theme, ThemeMode, TitleBar};
use gpui_kit::*;

use app::tray_icon::{self, QuitKeepContainers, QuitStopContainers, ShowMainWindow};
use app::{AppView, DesktopShell};

actions!(wslc_desktop, [Quit]);

fn main() {
    // Keep the mutex alive for the process lifetime (dropped after `app.run`).
    let _instance = match single_instance::try_acquire() {
        Ok(lock) => Some(lock),
        Err(single_instance::AcquireError::AlreadyRunning) => {
            // Focus the running window (including tray-hidden) instead of a dialog.
            single_instance::activate_existing_instance();
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

        tray_icon::install(cx);

        cx.bind_keys([KeyBinding::new("alt-f4", Quit, None)]);

        cx.on_action(|_: &Quit, cx: &mut App| {
            dispatch_close_request(cx);
        });
        cx.on_action(|_: &ShowMainWindow, cx: &mut App| {
            show_main_window(cx);
        });
        cx.on_action(|_: &QuitKeepContainers, cx: &mut App| {
            if let Some(shell) = cx.try_global::<DesktopShell>() {
                let view = shell.view.clone();
                let _ = view.update(cx, |this, cx| {
                    this.quit_keep_containers(cx);
                });
            } else {
                cx.quit();
            }
        });
        cx.on_action(|_: &QuitStopContainers, cx: &mut App| {
            if let Some(shell) = cx.try_global::<DesktopShell>() {
                let view = shell.view.clone();
                let _ = view.update(cx, |this, cx| {
                    this.begin_quit_and_stop_containers(cx);
                });
            } else {
                cx.quit();
            }
        });

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1080.), px(720.)), cx)),
            ..TitleBar::window_options()
        };

        let (window_handle, view) = gpui_kit::open_window(window_options, cx, |window, cx| {
            window.activate_window();
            window.set_window_title("WSLc Desktop");
            let view = cx.new(|cx| AppView::new(window, cx));
            let weak = view.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                weak.update(cx, |this, cx| this.handle_window_close_request(window, cx))
                    .unwrap_or(true)
            });
            view
        })
        .expect("Failed to open window");

        cx.set_global(DesktopShell {
            window: window_handle,
            view: view.downgrade(),
        });
    });
}

fn dispatch_close_request(cx: &mut App) {
    let Some(shell) = cx.try_global::<DesktopShell>() else {
        cx.quit();
        return;
    };
    let window = shell.window;
    let view = shell.view.clone();
    let _ = window.update(cx, |_, window, cx| {
        let _ = view.update(cx, |this, cx| {
            let _ = this.handle_window_close_request(window, cx);
        });
    });
}

fn show_main_window(cx: &mut App) {
    let Some(shell) = cx.try_global::<DesktopShell>() else {
        return;
    };
    let window = shell.window;
    let view = shell.view.clone();
    let _ = window.update(cx, |_, window, cx| {
        let _ = view.update(cx, |this, cx| {
            this.show_from_tray(window, cx);
        });
    });
}
