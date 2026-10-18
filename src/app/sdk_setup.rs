use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    progress::Progress,
    v_flex,
};
use gpui_kit::prelude::{FluentBuilder as _, InteractiveElement as _};
use gpui_kit::*;
use smol::Timer;

use crate::i18n::{self};
use crate::wslc::{
    EngineStatus, SdkSetupEvent, SetupFault, SetupStepId, SetupStepState, WslcClient, setup_log,
};

use super::AppView;
use super::poll::recv_all;
use super::state::{MAX_SDK_LOG_LINES, SdkSetupPhase};
use super::widgets::{modal_panel, modal_scrim, scroll_log_pane, status_badge};

impl AppView {
    pub(crate) fn append_sdk_log(&mut self, line: impl Into<SharedString>) {
        let line = line.into();
        if let Err(err) = setup_log::append_line(&line)
            && self.sdk_log_error.is_none()
        {
            self.sdk_log_error =
                Some(i18n::fmt1(self.t().sdk_setup_log_write_failed, "err", err).into());
        }
        self.sdk_logs_copied = false;
        self.sdk_setup_logs.push(line);
        if self.sdk_setup_logs.len() > MAX_SDK_LOG_LINES {
            let drop = self.sdk_setup_logs.len() - MAX_SDK_LOG_LINES;
            self.sdk_setup_logs.drain(0..drop);
        }
    }

    pub(crate) fn begin_sdk_setup(&mut self, force_redownload: bool, cx: &mut Context<Self>) {
        if self.sdk_setup_phase == Some(SdkSetupPhase::Running) {
            return;
        }

        if let Err(err) = setup_log::begin_session() {
            self.sdk_log_error =
                Some(i18n::fmt1(self.t().sdk_setup_log_write_failed, "err", err).into());
        } else {
            self.sdk_log_error = None;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        WslcClient::start_setup(force_redownload, tx);
        self.client = None;
        self.sdk_setup_rx = Some(rx);
        self.sdk_setup_phase = Some(SdkSetupPhase::Running);
        self.sdk_setup_progress = 0.0;
        self.sdk_setup_status = self.t().sdk_setup_installing.into();
        self.wsl_step = SetupStepState::Running;
        self.sdk_step = SetupStepState::Pending;
        self.sdk_logs_copied = false;
        self.sdk_setup_logs.clear();
        self.append_sdk_log(if force_redownload {
            self.t().sdk_setup_log_retry
        } else {
            self.t().sdk_setup_log_start
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

    fn apply_setup_step(&mut self, id: SetupStepId, state: SetupStepState) {
        let log_line = match &state {
            SetupStepState::Passed { detail } if !detail.is_empty() => {
                Some(self.step_passed_text(id, detail))
            }
            SetupStepState::Failed(fault) if !matches!(fault, SetupFault::Message(_)) => {
                Some(self.fault_text(fault))
            }
            _ => None,
        };
        match id {
            SetupStepId::WslVersion => self.wsl_step = state,
            SetupStepId::SdkInstall => self.sdk_step = state,
        }
        if let Some(line) = log_line {
            self.append_sdk_log(line);
        }
    }

    fn fail_unfinished_step(&mut self, fault: SetupFault) {
        if self.sdk_step.is_failed() || self.wsl_step.is_failed() {
            return;
        }
        if matches!(self.sdk_step, SetupStepState::Running)
            || matches!(self.wsl_step, SetupStepState::Passed { .. })
        {
            self.sdk_step = SetupStepState::Failed(fault);
        } else {
            self.wsl_step = SetupStepState::Failed(fault);
        }
    }

    fn current_step_failure_text(&self) -> Option<String> {
        if let SetupStepState::Failed(fault) = &self.wsl_step {
            return Some(self.fault_text(fault));
        }
        if let SetupStepState::Failed(fault) = &self.sdk_step {
            return Some(self.fault_text(fault));
        }
        None
    }

    fn fault_text(&self, fault: &SetupFault) -> String {
        let t = self.t();
        let required = crate::wslc::min_wsl_version_text();
        match fault {
            SetupFault::WslMissing => t.sdk_wsl_missing.to_string(),
            SetupFault::WslTooOld { found } => {
                i18n::fmt2(t.sdk_wsl_too_old, "found", found, "required", &required)
            }
            SetupFault::NotWindows => i18n::fmt1(t.sdk_not_windows, "required", &required),
            SetupFault::Message(message) => message.clone(),
        }
    }

    fn step_passed_text(&self, id: SetupStepId, detail: &str) -> String {
        let t = self.t();
        let required = crate::wslc::min_wsl_version_text();
        match id {
            SetupStepId::WslVersion => {
                i18n::fmt2(t.sdk_wsl_ok, "version", detail, "required", &required)
            }
            SetupStepId::SdkInstall => i18n::fmt1(t.sdk_install_ok, "version", detail),
        }
    }

    fn copy_sdk_logs(&mut self, cx: &mut Context<Self>) {
        if self.sdk_setup_logs.is_empty() {
            return;
        }
        let text = self
            .sdk_setup_logs
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.sdk_logs_copied = true;
        cx.notify();
    }

    fn open_sdk_log_folder(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = setup_log::open_log_dir() {
            self.sdk_log_error =
                Some(i18n::fmt1(self.t().sdk_setup_open_log_folder_failed, "err", err).into());
        }
        cx.notify();
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
                SdkSetupEvent::Step { id, state } => {
                    self.apply_setup_step(id, state);
                }
                SdkSetupEvent::Finished(result) => {
                    finished = true;
                    match result {
                        Ok(client) => {
                            self.client = Some(client);
                            self.sdk_setup_phase = None;
                            self.sdk_setup_progress = 100.0;
                            self.sdk_setup_status = self.t().ready.into();
                            self.append_sdk_log(self.t().sdk_setup_log_connected);
                            self.refresh(cx);
                        }
                        Err(err) => {
                            self.client = None;
                            self.sdk_setup_phase = Some(SdkSetupPhase::Failed);
                            if !self.wsl_step.is_failed() && !self.sdk_step.is_failed() {
                                self.fail_unfinished_step(SetupFault::Message(err.clone()));
                                self.append_sdk_log(format!("✗ {err}"));
                            }
                            self.status = EngineStatus::unavailable(err.clone());
                            self.status.message = self.localize_engine_message(&self.status);
                            self.sdk_setup_status = self.t().sdk_setup_failed.into();
                            self.status_line = self
                                .current_step_failure_text()
                                .unwrap_or_else(|| {
                                    i18n::fmt1(self.t().engine_unusable, "err", &err)
                                })
                                .into();
                        }
                    }
                }
            }
        }

        if disconnected && self.sdk_setup_phase == Some(SdkSetupPhase::Running) {
            let message = self.t().sdk_setup_channel_disconnected.to_string();
            self.sdk_setup_phase = Some(SdkSetupPhase::Failed);
            self.sdk_setup_status = message.clone().into();
            self.fail_unfinished_step(SetupFault::Message(message.clone()));
            self.append_sdk_log(message.clone());
            self.status = EngineStatus::unavailable(message.clone());
            self.status_line = message.into();
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

    fn render_setup_step(
        &self,
        id: SetupStepId,
        index: u8,
        title: &'static str,
        state: &SetupStepState,
        cx: &Context<Self>,
    ) -> Div {
        let t = self.t();
        let (badge, color) = match state {
            SetupStepState::Pending => (t.sdk_setup_step_pending, cx.theme().muted_foreground),
            SetupStepState::Running => (t.sdk_setup_step_checking, cx.theme().blue),
            SetupStepState::Passed { .. } => (t.sdk_setup_step_passed, cx.theme().green),
            SetupStepState::Failed(_) => (t.sdk_setup_step_failed, cx.theme().red),
        };
        let detail = match state {
            SetupStepState::Passed { detail } if !detail.is_empty() => Some((
                self.step_passed_text(id, detail),
                cx.theme().muted_foreground,
            )),
            SetupStepState::Failed(fault) => Some((self.fault_text(fault), cx.theme().red)),
            _ => None,
        };

        v_flex()
            .gap_1()
            .w_full()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_6()
                            .rounded_full()
                            .bg(color.opacity(0.18))
                            .text_color(color)
                            .text_xs()
                            .font_semibold()
                            .child(index.to_string()),
                    )
                    .child(div().flex_1().text_sm().font_medium().child(title))
                    .child(status_badge(badge, color, cx)),
            )
            .when_some(detail, |row, (text, color)| {
                row.child(div().pl_6().text_xs().text_color(color).child(text))
            })
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
        let logs = self.sdk_setup_logs.clone();
        let logs_open = self.sdk_logs_open;
        let copied = self.sdk_logs_copied;
        let log_path = setup_log::log_file_path();
        let log_error = self.sdk_log_error.clone();
        let wsl_step = self.wsl_step.clone();
        let sdk_step = self.sdk_step.clone();

        modal_scrim(
            "sdk-setup-modal",
            cx,
            modal_panel(cx)
                .id("sdk-setup-panel")
                .w(px(600.))
                .max_h(px(680.))
                .overflow_y_scroll()
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
                .child(self.render_setup_step(
                    SetupStepId::WslVersion,
                    1,
                    t.sdk_setup_step_wsl,
                    &wsl_step,
                    cx,
                ))
                .child(self.render_setup_step(
                    SetupStepId::SdkInstall,
                    2,
                    t.sdk_setup_step_sdk,
                    &sdk_step,
                    cx,
                ))
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
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("sdk-setup-view-logs")
                                .label(if logs_open {
                                    t.sdk_setup_hide_logs
                                } else {
                                    t.sdk_setup_view_logs
                                })
                                .when(failed, |button| button.primary())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sdk_logs_open = !this.sdk_logs_open;
                                    cx.notify();
                                })),
                        )
                        .when(failed, |row| {
                            row.child(
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
                .when(logs_open, |panel| {
                    panel
                        .child(
                            scroll_log_pane("sdk-setup-logs", 220., cx).child(
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
                                                .font_family("monospace")
                                                .text_color(cx.theme().foreground)
                                                .child(line)
                                        }))
                                    }
                                })),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("sdk-setup-copy-logs")
                                        .small()
                                        .label(if copied {
                                            t.sdk_setup_logs_copied
                                        } else {
                                            t.sdk_setup_copy_logs
                                        })
                                        .disabled(self.sdk_setup_logs.is_empty())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.copy_sdk_logs(cx);
                                        })),
                                )
                                .child(
                                    Button::new("sdk-setup-open-log-dir")
                                        .small()
                                        .label(t.sdk_setup_open_log_folder)
                                        .icon(IconName::Folder)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.open_sdk_log_folder(cx);
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{}: {}", t.sdk_setup_log_file, log_path.display())),
                        )
                        .when_some(log_error, |panel, err| {
                            panel.child(div().text_xs().text_color(cx.theme().red).child(err))
                        })
                }),
        )
    }
}
