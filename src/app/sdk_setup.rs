use std::sync::mpsc::Receiver;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, StyledExt as _,
    Theme, ThemeMode, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    label::Label,
    progress::Progress,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use gpui_kit::prelude::{FluentBuilder as _, InteractiveElement as _};
use gpui_kit::*;
use smol::Timer;

use crate::i18n::{self, Locale, Strings};
use crate::wslc::{
    BackendKind, ContainerInfo, CreateContainerRequest, EngineStatus, ImageInfo, PortMapping,
    PullEvent, SdkSetupEvent, VolumeInfo, VolumeMapping, WslcClient,
};

use super::state::{PullPhase, SdkSetupPhase, Section, MAX_PULL_LOG_LINES, MAX_SDK_LOG_LINES};
use super::AppView;
use super::poll::recv_all;
use super::widgets::{modal_panel, modal_scrim};

impl AppView {
    pub(crate) fn append_sdk_log(&mut self, line: impl Into<SharedString>) {
        self.sdk_setup_logs.push(line.into());
        if self.sdk_setup_logs.len() > MAX_SDK_LOG_LINES {
            let drop = self.sdk_setup_logs.len() - MAX_SDK_LOG_LINES;
            self.sdk_setup_logs.drain(0..drop);
        }
    }

    pub(crate) fn begin_sdk_setup(&mut self, force_redownload: bool, cx: &mut Context<Self>) {
        if self.sdk_setup_phase == Some(SdkSetupPhase::Running) {
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        WslcClient::start_setup(force_redownload, tx);
        self.client = None;
        self.sdk_setup_rx = Some(rx);
        self.sdk_setup_phase = Some(SdkSetupPhase::Running);
        self.sdk_setup_progress = 0.0;
        self.sdk_setup_status = self.t().sdk_setup_installing.into();
        self.sdk_setup_logs.clear();
        self.append_sdk_log(if force_redownload {
            "Retry: reinstalling WSL Containers SDK…"
        } else {
            "Starting WSL Containers SDK setup…"
        });
        self.status = EngineStatus::unavailable(self.t().sdk_locked);
        self.status_line = self.t().sdk_locked.into();
        self.sdk_setup_poll_generation = self.sdk_setup_poll_generation.wrapping_add(1);
        let generation = self.sdk_setup_poll_generation;
        cx.notify();

        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(80)).await;
                let cont = this
                    .update(cx, |this, cx| {
                        if this.sdk_setup_poll_generation != generation {
                            return false;
                        }
                        this.poll_sdk_setup(cx)
                    })
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        })
        .detach();
    }

    /// Returns `true` while setup is still running.
    pub(crate) fn poll_sdk_setup(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(rx) = self.sdk_setup_rx.take() else {
            return false;
        };

        let (events, disconnected) = recv_all(&rx);

        let mut finished = disconnected;
        for event in events {
            match event {
                SdkSetupEvent::Status(status) => {
                    self.sdk_setup_status = status.into();
                }
                SdkSetupEvent::Progress(value) => {
                    self.sdk_setup_progress = value.clamp(0.0, 100.0);
                }
                SdkSetupEvent::Log(line) => {
                    self.append_sdk_log(line);
                }
                SdkSetupEvent::Finished(result) => {
                    finished = true;
                    match result {
                        Ok(client) => {
                            self.client = Some(client);
                            self.sdk_setup_phase = None;
                            self.sdk_setup_progress = 100.0;
                            self.sdk_setup_status = self.t().ready.into();
                            self.append_sdk_log("✓ SDK connected");
                            self.refresh(cx);
                        }
                        Err(err) => {
                            self.client = None;
                            self.sdk_setup_phase = Some(SdkSetupPhase::Failed);
                            self.status = EngineStatus::unavailable(err.clone());
                            self.status.message = self.localize_engine_message(&self.status);
                            let msg = i18n::fmt1(self.t().engine_unusable, "err", &err);
                            self.sdk_setup_status = self.t().sdk_setup_failed.into();
                            self.append_sdk_log(format!("✗ {err}"));
                            self.status_line = msg.into();
                        }
                    }
                }
            }
        }

        if disconnected && self.sdk_setup_phase == Some(SdkSetupPhase::Running) {
            self.sdk_setup_phase = Some(SdkSetupPhase::Failed);
            self.sdk_setup_status = self.t().sdk_setup_channel_disconnected.into();
            self.append_sdk_log(self.t().sdk_setup_channel_disconnected);
            self.status = EngineStatus::unavailable(self.t().sdk_setup_channel_disconnected);
            finished = true;
        }

        if finished {
            self.sdk_setup_rx = None;
        } else {
            self.sdk_setup_rx = Some(rx);
        }
        cx.notify();
        self.sdk_setup_phase == Some(SdkSetupPhase::Running)
    }
    pub(crate) fn render_sdk_setup_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let phase = self.sdk_setup_phase.unwrap_or(SdkSetupPhase::Running);
        let failed = phase == SdkSetupPhase::Failed;
        let badge_color = if failed {
            cx.theme().red
        } else {
            cx.theme().blue
        };
        let logs = self
            .sdk_setup_logs
            .iter()
            .cloned()
            .collect::<Vec<SharedString>>();

        modal_scrim(
            "sdk-setup-modal",
            cx,
            modal_panel(cx)
                .w(px(560.))
                .max_h(px(520.))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(div().font_semibold().text_base().child(t.sdk_setup_title))
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded(cx.theme().radius)
                                    .bg(badge_color.opacity(0.18))
                                    .text_color(badge_color)
                                    .text_xs()
                                    .font_semibold()
                                    .child(phase.label(t)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.sdk_setup_intro),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(self.sdk_setup_status.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .child(format!("{:.0}%", self.sdk_setup_progress)),
                            ),
                    )
                    .child(
                        Progress::new("sdk-setup-progress")
                            .w_full()
                            .h_2()
                            .value(self.sdk_setup_progress),
                    )
                    .child(
                        div()
                            .id("sdk-setup-logs")
                            .h(px(220.))
                            .w_full()
                            .overflow_y_scroll()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background.opacity(0.65))
                            .p_2()
                            .child(
                                v_flex().gap_0p5().w_full().map(|list| {
                                    if logs.is_empty() {
                                        list.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(t.sdk_setup_logs_placeholder),
                                        )
                                    } else {
                                        list.children(logs.into_iter().map(|line| {
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(line)
                                        }))
                                    }
                                }),
                            ),
                    )
                    .when(failed, |el| {
                        el.child(
                            Button::new("sdk-setup-retry")
                                .primary()
                                .label(t.sdk_setup_retry)
                                .icon(IconName::RefreshCw)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.begin_sdk_setup(true, cx);
                                })),
                        )
                    }),
            )
    }
}
