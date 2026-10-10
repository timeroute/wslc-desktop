
use gpui_kit::component::{
    Theme, ThemeMode,
};
use gpui_kit::*;

use crate::i18n::{self, Locale};
use crate::wslc::{
    BackendKind, EngineStatus, WslcClient,
};

use super::state::PullPhase;
use super::AppView;

impl AppView {
    pub(crate) fn apply_locale(&mut self, locale: Locale, window: &mut Window, cx: &mut Context<Self>) {
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
        self.command_input.update(cx, |state, cx| {
            state.set_placeholder(t.command_placeholder, window, cx);
        });
        self.volume_input.update(cx, |state, cx| {
            state.set_placeholder(t.volume_placeholder, window, cx);
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

    pub(crate) fn apply_theme(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<Self>) {
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
                    if detail.is_empty() || lower.contains("wslcsdk.dll") || lower.contains("sdk not found")
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
                self.status_line = i18n::fmt1(t.list_containers_failed, "err", err.to_string()).into();
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
                if self.selected_volume.is_some_and(|i| i >= self.volumes.len()) {
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

    pub(crate) fn with_busy(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(&WslcClient) -> anyhow::Result<String>,
    ) {
        if self.sdk_locked() || self.busy || self.pull_phase == PullPhase::Pulling {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        self.busy = true;
        cx.notify();
        match op(&client) {
            Ok(msg) => {
                self.busy = false;
                self.set_status(msg, cx);
                self.refresh(cx);
            }
            Err(err) => {
                self.busy = false;
                let msg = i18n::fmt1(self.t().error_prefix, "err", err.to_string());
                self.set_status(msg, cx);
            }
        }
    }
}
