//! System tray resident icon (Windows). Non-Windows builds are no-ops.

use gpui_kit::*;

/// Actions dispatched from the tray menu / tray activation.
pub mod actions {
    use gpui_kit::actions;
    actions!(
        wslc_desktop_tray,
        [ShowMainWindow, QuitKeepContainers, QuitStopContainers]
    );
}

pub use actions::{QuitKeepContainers, QuitStopContainers, ShowMainWindow};

#[cfg(windows)]
mod windows_tray {
    use std::sync::Arc;

    use gpui_kit::{App, Global, MenuItem, QuitMode};
    use gpui_tray::{Icon, Tray};

    use super::{QuitKeepContainers, QuitStopContainers, ShowMainWindow};
    use crate::i18n::Locale;

    pub struct AppTray(pub Tray);

    impl Global for AppTray {}

    pub fn install(cx: &mut App) {
        cx.set_quit_mode(QuitMode::Explicit);

        let locale = Locale::from_system();
        let tooltip = locale.strings().tray_tooltip;
        match Tray::builder()
            .icon(make_icon())
            .tooltip(tooltip)
            .on_activate(ShowMainWindow)
            .menu(build_menu)
            .build(cx)
        {
            Ok(tray) => cx.set_global(AppTray(tray)),
            Err(err) => {
                // Tray is best-effort; close-prompt / minimize still work via taskbar minimize.
                eprintln!("WSLc Desktop: tray icon unavailable: {err}");
            }
        }
    }

    fn build_menu(cx: &mut App) -> Vec<MenuItem> {
        let locale = Locale::from_system();
        // Prefer the live UI locale when the main view already chose one.
        let t = if let Some(shell) = cx.try_global::<crate::app::DesktopShell>() {
            shell
                .view
                .upgrade()
                .map(|view| view.read(cx).t())
                .unwrap_or_else(|| locale.strings())
        } else {
            locale.strings()
        };
        vec![
            MenuItem::action(t.tray_show_window, ShowMainWindow),
            MenuItem::separator(),
            MenuItem::action(t.tray_quit_keep, QuitKeepContainers),
            MenuItem::action(t.tray_quit_stop, QuitStopContainers),
        ]
    }

    fn make_icon() -> Icon {
        // Simple 32×32 teal mark so we do not need image decoding at startup.
        const SIZE: u32 = 32;
        let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
        for y in 0..SIZE {
            for x in 0..SIZE {
                let i = ((y * SIZE + x) * 4) as usize;
                let edge = x < 2 || y < 2 || x >= SIZE - 2 || y >= SIZE - 2;
                let inner = x >= 8 && x < 24 && y >= 8 && y < 24;
                if edge {
                    rgba[i] = 0x0f;
                    rgba[i + 1] = 0x76;
                    rgba[i + 2] = 0x6e;
                    rgba[i + 3] = 0xff;
                } else if inner {
                    rgba[i] = 0x14;
                    rgba[i + 1] = 0xb8;
                    rgba[i + 2] = 0xa6;
                    rgba[i + 3] = 0xff;
                } else {
                    rgba[i] = 0x13;
                    rgba[i + 1] = 0x42;
                    rgba[i + 2] = 0x52;
                    rgba[i + 3] = 0xff;
                }
            }
        }
        Icon::from_rgba(Arc::<[u8]>::from(rgba), SIZE, SIZE).expect("tray icon rgba")
    }
}

#[cfg(windows)]
pub use windows_tray::install;

#[cfg(not(windows))]
pub fn install(_cx: &mut App) {}

/// Refresh tray menu labels after a locale change (Windows only).
pub fn refresh_menu(cx: &mut App) {
    #[cfg(windows)]
    {
        use gpui_kit::Global as _;
        if let Some(tray) = cx.try_global::<windows_tray::AppTray>() {
            let tray = tray.0.clone();
            let _ = tray.refresh_menu(cx);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = cx;
    }
}
