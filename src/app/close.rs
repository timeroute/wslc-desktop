
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::*;
use smol::Timer;

use crate::i18n::{self};
use crate::wslc::{CloseBehavior, WslcClient};

use super::widgets::{modal_panel, modal_scrim};
use super::AppView;

impl AppView {
    /// Window close / Alt+F4 path. Returns `false` to keep the window open.
    pub(crate) fn handle_window_close_request(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.allow_window_close {
            return true;
        }
        if self.close_prompt_open {
            return false;
        }
        match self.session_prefs.close_behavior {
            CloseBehavior::Ask => {
                self.close_prompt_open = true;
                cx.notify();
                false
            }
            CloseBehavior::MinimizeToTray => {
                self.minimize_to_tray(window, cx);
                false
            }
            CloseBehavior::QuitAndStopContainers => {
                self.begin_quit_and_stop_containers(cx);
                false
            }
        }
    }

    pub(crate) fn dismiss_close_prompt(&mut self, cx: &mut Context<Self>) {
        self.close_prompt_open = false;
        cx.notify();
    }

    pub(crate) fn choose_minimize_to_tray(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.session_prefs.close_behavior = CloseBehavior::MinimizeToTray;
        let _ = self.persist_close_behavior(cx);
        self.close_prompt_open = false;
        self.minimize_to_tray(window, cx);
    }

    pub(crate) fn choose_quit_and_stop(&mut self, cx: &mut Context<Self>) {
        self.session_prefs.close_behavior = CloseBehavior::QuitAndStopContainers;
        let _ = self.persist_close_behavior(cx);
        self.close_prompt_open = false;
        self.begin_quit_and_stop_containers(cx);
    }

    pub(crate) fn set_close_behavior(
        &mut self,
        behavior: CloseBehavior,
        cx: &mut Context<Self>,
    ) {
        self.session_prefs.close_behavior = behavior;
        match self.persist_close_behavior(cx) {
            Ok(()) => self.set_status(self.t().settings_close_saved, cx),
            Err(err) => {
                let msg = i18n::fmt1(self.t().error_prefix, "err", err);
                self.set_status(msg, cx);
            }
        }
    }

    fn persist_close_behavior(&mut self, _cx: &mut Context<Self>) -> Result<(), String> {
        let prefs = self.session_prefs.clone().clamp();
        self.session_prefs = prefs.clone();
        WslcClient::save_session_prefs(prefs).map_err(|e| format!("{e}"))
    }

    pub(crate) fn minimize_to_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_prompt_open = false;
        self.window_hidden = true;
        hide_native_window(window);
        self.set_status(self.t().minimized_to_tray, cx);
        cx.notify();
    }

    pub(crate) fn show_from_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window_hidden = false;
        show_native_window(window);
        window.activate_window();
        cx.notify();
    }

    /// Quit the UI process without stopping containers (tray / explicit keep).
    pub(crate) fn quit_keep_containers(&mut self, cx: &mut Context<Self>) {
        self.allow_window_close = true;
        self.close_prompt_open = false;
        cx.quit();
    }

    pub(crate) fn begin_quit_and_stop_containers(&mut self, cx: &mut Context<Self>) {
        self.close_prompt_open = false;
        let Some(client) = self.client.clone() else {
            self.allow_window_close = true;
            cx.quit();
            return;
        };

        if self.busy {
            // Already mid-op; still quit after a best-effort stop on a fresh thread.
        }

        self.set_status(self.t().stopping_containers_on_quit, cx);
        let template = self.t().quit_stopped_containers;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("wslc-quit-stop".into())
            .spawn(move || {
                let result = client
                    .stop_all_running_containers()
                    .map(|count| i18n::fmt1(template, "count", count.to_string()));
                let _ = tx.send(result);
            })
            .expect("spawn quit-stop thread");

        self.busy = true;
        self.busy_rx = Some(rx);
        self.pending_quit_after_stop = true;
        self.busy_poll_generation = self.busy_poll_generation.wrapping_add(1);
        let generation = self.busy_poll_generation;
        cx.notify();

        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(50)).await;
                let cont = this
                    .update(cx, |this, cx| {
                        if this.busy_poll_generation != generation {
                            return false;
                        }
                        this.poll_busy(cx)
                    })
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn finish_quit_after_stop(&mut self, cx: &mut Context<Self>) {
        self.pending_quit_after_stop = false;
        self.allow_window_close = true;
        cx.quit();
    }

    pub(crate) fn render_close_prompt_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();

        modal_scrim(
            "close-prompt-modal",
            cx,
            modal_panel(cx)
                .w(px(480.))
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(div().font_semibold().text_base().child(t.close_prompt_title))
                        .child(
                            Button::new("close-prompt-dismiss")
                                .ghost()
                                .small()
                                .icon(IconName::Close)
                                .tooltip(t.close)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.dismiss_close_prompt(cx);
                                })),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.close_prompt_body),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            Button::new("close-prompt-minimize")
                                .label(t.close_prompt_minimize)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.choose_minimize_to_tray(window, cx);
                                })),
                        )
                        .child(
                            Button::new("close-prompt-quit-stop")
                                .danger()
                                .label(t.close_prompt_quit_stop)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.choose_quit_and_stop(cx);
                                })),
                        ),
                ),
        )
    }
}

fn hide_native_window(window: &mut Window) {
    #[cfg(windows)]
    {
        if let Some(hwnd) = native_hwnd(window) {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                    hwnd,
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
                );
            }
            return;
        }
    }
    window.minimize_window();
}

fn show_native_window(window: &mut Window) {
    #[cfg(windows)]
    {
        if let Some(hwnd) = native_hwnd(window) {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                    hwnd,
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW,
                );
            }
            return;
        }
    }
    window.activate_window();
}

#[cfg(windows)]
fn native_hwnd(window: &Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // UFCS: GPUI's `Window::window_handle()` returns `AnyWindowHandle` and shadows
    // the raw-window-handle trait method of the same name.
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get() as _),
        _ => None,
    }
}
