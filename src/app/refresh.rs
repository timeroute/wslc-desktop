use std::sync::mpsc;
use std::time::Duration;

use gpui_kit::component::{ActiveTheme, Theme, ThemeMode, h_flex, label::Label, v_flex};
use gpui_kit::*;
use smol::Timer;

use crate::i18n::{self, Locale};
use crate::wslc::{BackendKind, EngineStatus, WslcClient};

use super::AppView;
use super::poll::recv_all;
use super::state::PullPhase;
use super::widgets::modal_scrim;

impl AppView {
    pub(crate) fn apply_locale(
        &mut self,
        locale: Locale,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.locale == locale {
            return;
        }
        self.locale = locale;
        let t = self.t();
        self.name_input.update(cx, |state, cx| {
            state.set_placeholder(t.name_placeholder, window, cx);
        });
        self.ports_input.update(cx, |state, cx| {
            state.set_placeholder(t.ports_placeholder, window, cx);
        });
        self.bind_mounts_input.update(cx, |state, cx| {
            state.set_placeholder(t.bind_mounts_placeholder, window, cx);
        });
        self.named_volumes_input.update(cx, |state, cx| {
            state.set_placeholder(t.named_volumes_placeholder, window, cx);
        });
        self.command_input.update(cx, |state, cx| {
            state.set_placeholder(t.command_placeholder, window, cx);
        });
        self.template_name_input.update(cx, |state, cx| {
            state.set_placeholder(t.template_name_placeholder, window, cx);
        });
        self.template_rename_input.update(cx, |state, cx| {
            state.set_placeholder(t.template_name_placeholder, window, cx);
        });
        self.template_banner = None;
        self.volume_input.update(cx, |state, cx| {
            state.set_placeholder(t.volume_placeholder, window, cx);
        });
        self.exec_input.update(cx, |state, cx| {
            state.set_placeholder(t.container_exec_placeholder, window, cx);
        });
        if self.pull_phase == PullPhase::Idle {
            self.pull_status = t.pull_hint.into();
        }
        // Relocalize in-memory status only — do not re-probe the SDK.
        self.status.message = self.localize_engine_message(&self.status);
        if self.sdk_locked() {
            self.status_line = t.sdk_locked.into();
        } else {
            let mode = match self.status.backend {
                BackendKind::Live => t.mode_live,
                BackendKind::Unavailable => t.mode_unavailable,
            };
            self.status_line = i18n::fmt4(
                t.refreshed_summary,
                "containers",
                self.containers.len().to_string(),
                "images",
                self.images.len().to_string(),
                "volumes",
                self.volumes.len().to_string(),
                "mode",
                mode,
            )
            .into();
        }
        cx.notify();
    }

    pub(crate) fn apply_theme(
        &mut self,
        mode: ThemeMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.theme_mode == mode {
            return;
        }
        self.theme_mode = mode;
        Theme::change(mode, Some(window), cx);
        cx.notify();
    }

    pub(crate) fn localize_engine_message(&self, status: &EngineStatus) -> String {
        let t = self.t();
        match status.backend {
            BackendKind::Live => i18n::fmt1(t.engine_connected, "bin", &status.version),
            BackendKind::Unavailable => {
                let detail = status.message.as_str();
                let lower = detail.to_ascii_lowercase();
                if lower.contains("sdk_needs_update") {
                    t.engine_sdk_outdated.to_string()
                } else if lower.contains("missing wsl components")
                    || lower.contains("virtual_machine_platform")
                    || lower.contains("wsl_package")
                {
                    i18n::fmt1(t.engine_missing_components, "err", detail)
                } else if detail.is_empty()
                    || lower.contains("sdk not found")
                    || lower.contains("wslcsdk.dll")
                    || lower.contains("sdk install failed")
                {
                    if detail.is_empty()
                        || lower.contains("wslcsdk.dll")
                        || lower.contains("sdk not found")
                    {
                        t.engine_missing.to_string()
                    } else {
                        i18n::fmt1(t.engine_unusable, "err", detail)
                    }
                } else {
                    i18n::fmt1(t.engine_unusable, "err", detail)
                }
            }
        }
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        let t = self.t();
        let Some(client) = self.client.clone() else {
            self.status = EngineStatus::unavailable(t.sdk_locked);
            self.status.message = self.localize_engine_message(&self.status);
            self.status_line = t.sdk_locked.into();
            cx.notify();
            return;
        };

        self.status = client.status();
        self.status.message = self.localize_engine_message(&self.status);

        // Live session dropped — lock the UI and force SDK setup again.
        if self.status.backend != BackendKind::Live {
            self.client = None;
            self.containers.clear();
            self.images.clear();
            self.volumes.clear();
            self.selected_container = None;
            self.selected_image = None;
            self.selected_volume = None;
            self.status_line = t.sdk_locked.into();
            if self.sdk_setup_phase.is_none() {
                self.begin_sdk_setup(false, cx);
            } else {
                cx.notify();
            }
            return;
        }

        let mut ok = true;
        match client.list_containers(self.show_all_containers) {
            Ok(rows) => {
                self.containers = rows;
                if self
                    .selected_container
                    .is_some_and(|i| i >= self.containers.len())
                {
                    self.selected_container = None;
                }
            }
            Err(err) => {
                ok = false;
                self.status_line =
                    i18n::fmt1(t.list_containers_failed, "err", err.to_string()).into();
            }
        }
        match client.list_images() {
            Ok(rows) => {
                self.images = rows;
                if self.selected_image.is_some_and(|i| i >= self.images.len()) {
                    self.selected_image = None;
                }
            }
            Err(err) => {
                ok = false;
                self.status_line = i18n::fmt1(t.list_images_failed, "err", err.to_string()).into();
            }
        }
        match client.list_volumes() {
            Ok(rows) => {
                self.volumes = rows;
                if self
                    .selected_volume
                    .is_some_and(|i| i >= self.volumes.len())
                {
                    self.selected_volume = None;
                }
            }
            Err(err) => {
                ok = false;
                self.status_line = i18n::fmt1(t.list_volumes_failed, "err", err.to_string()).into();
            }
        }
        if ok {
            self.status_line = i18n::fmt4(
                t.refreshed_summary,
                "containers",
                self.containers.len().to_string(),
                "images",
                self.images.len().to_string(),
                "volumes",
                self.volumes.len().to_string(),
                "mode",
                t.mode_live,
            )
            .into();
        }
        cx.notify();
    }

    /// Run an SDK call off the UI thread and show a busy overlay until it finishes.
    pub(crate) fn with_busy(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(&WslcClient) -> anyhow::Result<String> + Send + 'static,
    ) {
        if self.sdk_locked() || self.busy || self.pull_phase == PullPhase::Pulling {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };

        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("wslc-ui-op".into())
            .spawn(move || {
                let result = op(&client);
                let _ = tx.send(result);
            })
            .expect("spawn UI op thread");

        self.busy = true;
        self.busy_rx = Some(rx);
        self.status_line = self.t().working.into();
        self.busy_poll_generation = self.busy_poll_generation.wrapping_add(1);
        let generation = self.busy_poll_generation;
        cx.notify();

        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(50)).await;
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

    /// Returns `true` while the background op is still running.
    fn poll_busy(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(rx) = self.busy_rx.take() else {
            return false;
        };
        let (events, disconnected) = recv_all(&rx);
        if let Some(result) = events.into_iter().last() {
            self.busy = false;
            self.busy_rx = None;
            match result {
                Ok(msg) if msg.starts_with("__LOGS__\0") => {
                    let rest = &msg["__LOGS__\0".len()..];
                    if let Some((title, text)) = rest.split_once('\0') {
                        self.logs_title = title.to_string().into();
                        self.logs_text = text.to_string().into();
                        self.logs_modal_open = true;
                        self.status_line = self.t().ready.into();
                    } else {
                        self.set_status(msg, cx);
                    }
                    cx.notify();
                }
                Ok(msg) => {
                    if self.create_modal_open {
                        self.create_modal_open = false;
                    }
                    self.set_status(msg, cx);
                    self.refresh(cx);
                }
                Err(err) => {
                    let msg = i18n::fmt1(self.t().error_prefix, "err", err.to_string());
                    self.set_status(msg, cx);
                }
            }
            cx.notify();
            return false;
        }
        if disconnected {
            self.busy = false;
            self.busy_rx = None;
            self.set_status(self.t().op_channel_disconnected, cx);
            cx.notify();
            return false;
        }
        self.busy_rx = Some(rx);
        true
    }

    pub(crate) fn render_busy_overlay(&self, cx: &Context<Self>) -> impl IntoElement {
        let t = self.t();
        modal_scrim(
            "busy-overlay",
            cx,
            h_flex()
                .items_center()
                .justify_center()
                .px_6()
                .py_4()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .shadow_md()
                .child(
                    v_flex()
                        .gap_1()
                        .items_center()
                        .child(Label::new(t.working).text_sm()),
                ),
        )
    }
}
