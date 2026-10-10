use gpui_kit::*;

use crate::i18n::{Locale, Strings};
use crate::wslc::{ContainerInfo, ImageInfo, VolumeInfo};

pub(crate) const MAX_PULL_LOG_LINES: usize = 400;
pub(crate) const MAX_SDK_LOG_LINES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Containers = 0,
    Images = 1,
    Volumes = 2,
    Settings = 3,
}

impl Section {
    pub(crate) fn label(self, t: &Strings) -> &'static str {
        match self {
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
            || (self.create_modal_open && self.busy)
    }

    pub(crate) fn container_key(item: &ContainerInfo) -> String {
        if item.name.is_empty() {
            item.id.clone()
        } else {
            item.name.clone()
        }
    }

    pub(crate) fn selected_container(&self) -> Option<&ContainerInfo> {
        self.selected_container
            .and_then(|i| self.containers.get(i))
    }

    pub(crate) fn selected_image(&self) -> Option<&ImageInfo> {
        self.selected_image.and_then(|i| self.images.get(i))
    }

    pub(crate) fn selected_volume(&self) -> Option<&VolumeInfo> {
        self.selected_volume.and_then(|i| self.volumes.get(i))
    }

    pub(crate) fn t(&self) -> &'static Strings {
        self.locale.strings()
    }

    pub(crate) fn set_status(&mut self, msg: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status_line = msg.into();
        cx.notify();
    }
}
