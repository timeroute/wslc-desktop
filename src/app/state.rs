use gpui_kit::*;

use crate::i18n::Strings;
use crate::wslc::ContainerInfo;

pub(crate) const MAX_PULL_LOG_LINES: usize = 400;
pub(crate) const MAX_SDK_LOG_LINES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Overview = 0,
    Containers = 1,
    Images = 2,
    Volumes = 3,
    Settings = 4,
}

impl Section {
    pub(crate) fn label(self, t: &Strings) -> &'static str {
        match self {
            Self::Overview => t.overview,
            Self::Containers => t.containers,
            Self::Images => t.images,
            Self::Volumes => t.volumes,
            Self::Settings => t.settings,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PullPhase {
    #[default]
    Idle,
    Pulling,
    Success,
    Failed,
}

impl PullPhase {
    pub(crate) fn label(self, t: &Strings) -> &'static str {
        match self {
            Self::Idle => t.pull_idle,
            Self::Pulling => t.pull_phase_pulling,
            Self::Success => t.pull_ok,
            Self::Failed => t.pull_failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SdkSetupPhase {
    Running,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfirmAction {
    RemoveContainer {
        key: String,
        label: String,
        /// Session named volumes attached to this container (may be empty).
        named_volumes: Vec<String>,
        /// When true, also delete `named_volumes` after the container is removed.
        remove_volumes: bool,
    },
    RemoveImage { reference: String },
    RemoveVolume { name: String },
}

impl SdkSetupPhase {
    pub(crate) fn label(self, t: &Strings) -> &'static str {
        match self {
            Self::Running => t.sdk_setup_phase_running,
            Self::Failed => t.sdk_setup_phase_failed,
        }
    }
}

impl super::AppView {
    pub(crate) fn sdk_locked(&self) -> bool {
        self.sdk_setup_phase.is_some() || self.client.is_none()
    }

    pub(crate) fn ops_disabled(&self) -> bool {
        self.sdk_locked()
            || self.busy
            || self.pull_phase == PullPhase::Pulling
            || self.logs_modal_open
            || self.exec_modal_open
            || self.confirm.is_some()
            || self.close_prompt_open
            || self.volume_modal_open
            || (self.create_modal_open && self.busy)
    }

    pub(crate) fn container_key(item: &ContainerInfo) -> String {
        if item.name.is_empty() {
            item.id.clone()
        } else {
            item.name.clone()
        }
    }

    pub(crate) fn t(&self) -> &'static Strings {
        self.locale.strings()
    }

    pub(crate) fn set_status(&mut self, msg: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status_line = msg.into();
        cx.notify();
    }
}
