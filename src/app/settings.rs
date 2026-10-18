
use gpui_kit::component::{
    ActiveTheme, Sizable as _, StyledExt as _, ThemeMode,
    button::{Button, ButtonVariants as _},
    h_flex,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::i18n::Locale;
use crate::wslc::{CloseBehavior, SessionPrefs, WslcClient};

use super::widgets::settings_card;
use super::AppView;

const CPU_CHOICES: [u32; 5] = [1, 2, 4, 6, 8];
const MEMORY_CHOICES_MB: [u32; 5] = [1024, 2048, 4096, 8192, 16384];

impl AppView {
    pub(crate) fn save_session_prefs(&mut self, cx: &mut Context<Self>) {
        let prefs = self.session_prefs.clone().clamp();
        self.session_prefs = prefs.clone();
        match WslcClient::save_session_prefs(prefs) {
            Ok(()) => self.set_status(self.t().settings_prefs_saved, cx),
            Err(err) => {
                let msg = crate::i18n::fmt1(self.t().error_prefix, "err", err.to_string());
                self.set_status(msg, cx);
            }
        }
    }

    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let locale = self.locale;
        let theme_mode = self.theme_mode;
        let cpu = self.session_prefs.cpu_count;
        let memory = self.session_prefs.memory_mb;
        let close_behavior = self.session_prefs.close_behavior;

        div()
            .id("settings-panel")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .gap_6()
                    .max_w(px(560.))
                    .pb_6()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.settings_intro),
                    )
                    .child(
                        settings_card(cx)
                            .child(div().font_medium().text_sm().child(t.language))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.language_hint),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("lang-zh")
                                            .small()
                                            .label(Locale::Zh.label())
                                            .when(locale == Locale::Zh, |b| b.primary())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.apply_locale(Locale::Zh, window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("lang-en")
                                            .small()
                                            .label(Locale::En.label())
                                            .when(locale == Locale::En, |b| b.primary())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.apply_locale(Locale::En, window, cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        settings_card(cx)
                            .child(div().font_medium().text_sm().child(t.theme))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.theme_hint),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("theme-dark")
                                            .small()
                                            .label(t.theme_dark)
                                            .when(theme_mode == ThemeMode::Dark, |b| b.primary())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.apply_theme(ThemeMode::Dark, window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-light")
                                            .small()
                                            .label(t.theme_light)
                                            .when(theme_mode == ThemeMode::Light, |b| b.primary())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.apply_theme(ThemeMode::Light, window, cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        settings_card(cx)
                            .child(div().font_medium().text_sm().child(t.settings_close))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.settings_close_hint),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .flex_wrap()
                                    .child(
                                        Button::new("close-ask")
                                            .small()
                                            .label(t.settings_close_ask)
                                            .when(close_behavior == CloseBehavior::Ask, |b| {
                                                b.primary()
                                            })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_close_behavior(CloseBehavior::Ask, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("close-minimize")
                                            .small()
                                            .label(t.settings_close_minimize)
                                            .when(
                                                close_behavior == CloseBehavior::MinimizeToTray,
                                                |b| b.primary(),
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_close_behavior(
                                                    CloseBehavior::MinimizeToTray,
                                                    cx,
                                                );
                                            })),
                                    )
                                    .child(
                                        Button::new("close-quit-stop")
                                            .small()
                                            .label(t.settings_close_quit_stop)
                                            .when(
                                                close_behavior
                                                    == CloseBehavior::QuitAndStopContainers,
                                                |b| b.primary(),
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.set_close_behavior(
                                                    CloseBehavior::QuitAndStopContainers,
                                                    cx,
                                                );
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        settings_card(cx)
                            .child(div().font_medium().text_sm().child(t.settings_session))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.settings_session_hint),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.settings_cpu),
                            )
                            .child(
                                h_flex().gap_2().children(CPU_CHOICES.into_iter().map(|n| {
                                    Button::new(("settings-cpu", n as usize))
                                        .small()
                                        .label(n.to_string())
                                        .when(cpu == n, |b| b.primary())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.session_prefs.cpu_count = n;
                                            this.save_session_prefs(cx);
                                        }))
                                        .into_any_element()
                                })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.settings_memory),
                            )
                            .child(
                                h_flex().gap_2().children(MEMORY_CHOICES_MB.into_iter().map(
                                    |mb| {
                                        Button::new(("settings-mem", mb as usize))
                                            .small()
                                            .label(format!("{} MiB", mb))
                                            .when(memory == mb, |b| b.primary())
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.session_prefs.memory_mb = mb;
                                                this.save_session_prefs(cx);
                                            }))
                                            .into_any_element()
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{} · {} CPU · {} MiB",
                                        SessionPrefs::path().display(),
                                        self.session_prefs.cpu_count,
                                        self.session_prefs.memory_mb
                                    )),
                            ),
                    ),
            )
    }
}
