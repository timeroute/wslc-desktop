use std::sync::mpsc;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Sizable as _, StyledExt as _, ThemeMode,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use smol::Timer;

use crate::i18n::{self, Locale};
use crate::wslc::{
    APP_VERSION, CloseBehavior, GITHUB_REPO_URL, HOMEPAGE_URL, SessionPrefs, UpdateStatus,
    WslcClient, check_latest, open_url, releases_url,
};

use super::state::UpdateCheckPhase;
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
                let msg = i18n::fmt1(self.t().error_prefix, "err", err.to_string());
                self.set_status(msg, cx);
            }
        }
    }

    /// Open Settings and kick off a GitHub release check if needed.
    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.section = super::state::Section::Settings;
        self.ensure_update_check(cx);
        cx.notify();
    }

    /// Start a release check when idle / after a previous failure.
    pub(crate) fn ensure_update_check(&mut self, cx: &mut Context<Self>) {
        match &self.update_check {
            UpdateCheckPhase::Idle | UpdateCheckPhase::Failed { .. } => {
                self.begin_update_check(cx);
            }
            UpdateCheckPhase::Checking
            | UpdateCheckPhase::UpToDate { .. }
            | UpdateCheckPhase::Available { .. } => {}
        }
    }

    pub(crate) fn begin_update_check(&mut self, cx: &mut Context<Self>) {
        if matches!(self.update_check, UpdateCheckPhase::Checking) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("wslc-update-check".into())
            .spawn(move || {
                let _ = tx.send(check_latest(APP_VERSION));
            })
            .expect("spawn update-check thread");

        self.update_check = UpdateCheckPhase::Checking;
        self.update_check_poll_generation = self.update_check_poll_generation.wrapping_add(1);
        let generation = self.update_check_poll_generation;
        cx.notify();

        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(50)).await;
                let cont = this
                    .update(cx, |this, cx| {
                        if this.update_check_poll_generation != generation {
                            return false;
                        }
                        match rx.try_recv() {
                            Ok(Ok(UpdateStatus::UpToDate { latest })) => {
                                this.update_check = UpdateCheckPhase::UpToDate { latest };
                                cx.notify();
                                false
                            }
                            Ok(Ok(UpdateStatus::Available { latest, url })) => {
                                this.update_check = UpdateCheckPhase::Available { latest, url };
                                cx.notify();
                                false
                            }
                            Ok(Err(err)) => {
                                this.update_check = UpdateCheckPhase::Failed { message: err };
                                cx.notify();
                                false
                            }
                            Err(mpsc::TryRecvError::Empty) => true,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                this.update_check = UpdateCheckPhase::Failed {
                                    message: "channel disconnected".into(),
                                };
                                cx.notify();
                                false
                            }
                        }
                    })
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        })
        .detach();
    }

    fn open_external_url(&mut self, url: &str, cx: &mut Context<Self>) {
        match open_url(url) {
            Ok(()) => {}
            Err(err) => {
                let msg = i18n::fmt1(self.t().settings_about_open_failed, "err", err);
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
        let update_status_line = match &self.update_check {
            UpdateCheckPhase::Idle => String::new(),
            UpdateCheckPhase::Checking => t.settings_about_checking.to_string(),
            UpdateCheckPhase::UpToDate { latest } => {
                i18n::fmt1(t.settings_about_up_to_date, "version", latest.clone())
            }
            UpdateCheckPhase::Available { latest, .. } => {
                i18n::fmt1(t.settings_about_update_available, "version", latest.clone())
            }
            UpdateCheckPhase::Failed { message } => {
                i18n::fmt1(t.settings_about_check_failed, "err", message.clone())
            }
        };
        let update_available_url = match &self.update_check {
            UpdateCheckPhase::Available { url, .. } => Some(url.clone()),
            _ => None,
        };
        let checking = matches!(self.update_check, UpdateCheckPhase::Checking);

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
                    )
                    .child(
                        settings_card(cx)
                            .child(div().font_medium().text_sm().child(t.settings_about))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.settings_about_hint),
                            )
                            .child(
                                div().text_sm().child(format!(
                                    "{}  {}",
                                    t.settings_about_version, APP_VERSION
                                )),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .flex_wrap()
                                    .child(
                                        Button::new("about-github")
                                            .small()
                                            .label(t.settings_about_github)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_external_url(GITHUB_REPO_URL, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("about-homepage")
                                            .small()
                                            .label(t.settings_about_homepage)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_external_url(HOMEPAGE_URL, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("about-check")
                                            .small()
                                            .label(t.settings_about_check)
                                            .disabled(checking)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.begin_update_check(cx);
                                            })),
                                    )
                                    .when_some(update_available_url.clone(), |row, url| {
                                        row.child(
                                            Button::new("about-open-release")
                                                .small()
                                                .primary()
                                                .label(t.settings_about_open_release)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.open_external_url(&url, cx);
                                                })),
                                        )
                                    }),
                            )
                            .when(!update_status_line.is_empty(), |card| {
                                let accent = match &self.update_check {
                                    UpdateCheckPhase::Available { .. } => cx.theme().foreground,
                                    UpdateCheckPhase::Failed { .. } => cx.theme().red,
                                    UpdateCheckPhase::UpToDate { .. } => cx.theme().green,
                                    _ => cx.theme().muted_foreground,
                                };
                                card.child(
                                    div()
                                        .text_xs()
                                        .text_color(accent)
                                        .child(update_status_line),
                                )
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(releases_url()),
                            ),
                    ),
            )
    }
}
