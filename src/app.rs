use std::sync::mpsc::{Receiver, TryRecvError};
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

const MAX_PULL_LOG_LINES: usize = 400;
const MAX_SDK_LOG_LINES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Containers = 0,
    Images = 1,
    Volumes = 2,
    Settings = 3,
}

impl Section {
    fn label(self, t: &Strings) -> &'static str {
        match self {
            Self::Containers => t.containers,
            Self::Images => t.images,
            Self::Volumes => t.volumes,
            Self::Settings => t.settings,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PullPhase {
    #[default]
    Idle,
    Pulling,
    Success,
    Failed,
}

impl PullPhase {
    fn label(self, t: &Strings) -> &'static str {
        match self {
            Self::Idle => t.pull_idle,
            Self::Pulling => t.pull_phase_pulling,
            Self::Success => t.pull_ok,
            Self::Failed => t.pull_failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SdkSetupPhase {
    Running,
    Failed,
}

impl SdkSetupPhase {
    fn label(self, t: &Strings) -> &'static str {
        match self {
            Self::Running => t.sdk_setup_phase_running,
            Self::Failed => t.sdk_setup_phase_failed,
        }
    }
}

pub struct AppView {
    client: Option<WslcClient>,
    status: EngineStatus,
    section: Section,
    locale: Locale,
    theme_mode: ThemeMode,
    containers: Vec<ContainerInfo>,
    images: Vec<ImageInfo>,
    volumes: Vec<VolumeInfo>,
    selected_container: Option<usize>,
    selected_image: Option<usize>,
    selected_volume: Option<usize>,
    show_all_containers: bool,
    busy: bool,
    status_line: SharedString,
    image_input: Entity<InputState>,
    name_input: Entity<InputState>,
    ports_input: Entity<InputState>,
    bind_mounts_input: Entity<InputState>,
    command_input: Entity<InputState>,
    pull_input: Entity<InputState>,
    volume_input: Entity<InputState>,
    create_auto_remove: bool,
    pull_modal_open: bool,
    create_modal_open: bool,
    logs_modal_open: bool,
    logs_title: SharedString,
    logs_text: SharedString,
    pull_phase: PullPhase,
    pull_status: SharedString,
    pull_progress: f32,
    pull_logs: Vec<SharedString>,
    pull_rx: Option<Receiver<PullEvent>>,
    pull_poll_generation: u64,
    /// `Some` while the blocking SDK setup dialog must stay up.
    sdk_setup_phase: Option<SdkSetupPhase>,
    sdk_setup_status: SharedString,
    sdk_setup_progress: f32,
    sdk_setup_logs: Vec<SharedString>,
    sdk_setup_rx: Option<Receiver<SdkSetupEvent>>,
    sdk_setup_poll_generation: u64,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let locale = Locale::from_system();
        let t = locale.strings();
        // Theme is applied once in `main` before the window opens to avoid a
        // Light→Dark flash from gpui_kit::init's default Light theme.

        let image_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("nginx:alpine")
                .default_value("nginx:alpine")
        });
        let name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.name_placeholder));
        let ports_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t.ports_placeholder)
        });
        let bind_mounts_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t.bind_mounts_placeholder)
        });
        let command_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t.command_placeholder)
        });
        let pull_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("library/alpine:latest")
                .default_value("alpine:latest")
        });
        let volume_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.volume_placeholder));

        let mut view = Self {
            client: None,
            status: EngineStatus::unavailable(t.sdk_locked),
            section: Section::Containers,
            locale,
            theme_mode: ThemeMode::Dark,
            containers: Vec::new(),
            images: Vec::new(),
            volumes: Vec::new(),
            selected_container: None,
            selected_image: None,
            selected_volume: None,
            show_all_containers: true,
            busy: false,
            status_line: t.sdk_locked.into(),
            image_input,
            name_input,
            ports_input,
            bind_mounts_input,
            command_input,
            pull_input,
            volume_input,
            create_auto_remove: false,
            pull_modal_open: false,
            create_modal_open: false,
            logs_modal_open: false,
            logs_title: SharedString::default(),
            logs_text: SharedString::default(),
            pull_phase: PullPhase::Idle,
            pull_status: t.pull_hint.into(),
            pull_progress: 0.0,
            pull_logs: Vec::new(),
            pull_rx: None,
            pull_poll_generation: 0,
            sdk_setup_phase: None,
            sdk_setup_status: t.sdk_setup_installing.into(),
            sdk_setup_progress: 0.0,
            sdk_setup_logs: Vec::new(),
            sdk_setup_rx: None,
            sdk_setup_poll_generation: 0,
        };
        view.begin_sdk_setup(false, cx);
        view
    }

    fn sdk_locked(&self) -> bool {
        self.sdk_setup_phase.is_some() || self.client.is_none()
    }

    fn ops_disabled(&self) -> bool {
        self.sdk_locked()
            || self.busy
            || self.pull_phase == PullPhase::Pulling
            || self.logs_modal_open
            || (self.create_modal_open && self.busy)
    }

    fn container_key(item: &ContainerInfo) -> String {
        if item.name.is_empty() {
            item.id.clone()
        } else {
            item.name.clone()
        }
    }

    fn parse_port_mappings(raw: &str) -> Result<Vec<PortMapping>, String> {
        let mut out = Vec::new();
        for part in raw.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let Some((host_s, container_s)) = part.split_once(':') else {
                return Err(format!("expected host:container, got `{part}`"));
            };
            let host = host_s
                .trim()
                .parse::<u16>()
                .map_err(|_| format!("invalid host port in `{part}`"))?;
            let container = container_s
                .trim()
                .parse::<u16>()
                .map_err(|_| format!("invalid container port in `{part}`"))?;
            out.push(PortMapping { host, container });
        }
        Ok(out)
    }

    fn parse_bind_mounts(raw: &str) -> Result<Vec<VolumeMapping>, String> {
        let mut out = Vec::new();
        for part in raw.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let lower = part.to_ascii_lowercase();
            let read_only = lower.ends_with(":ro");
            let core = if read_only {
                part[..part.len() - 3].trim_end()
            } else {
                part
            };
            let Some(idx) = core.find(":/") else {
                return Err(format!("expected host:/container path, got `{part}`"));
            };
            let host = core[..idx].trim().to_string();
            let container = core[idx + 1..].trim().to_string();
            if host.is_empty() || container.is_empty() {
                return Err(format!("empty host or mount path in `{part}`"));
            }
            out.push(VolumeMapping {
                host,
                container,
                read_only,
            });
        }
        Ok(out)
    }

    fn open_pull_modal(&mut self, cx: &mut Context<Self>) {
        if self.sdk_locked() {
            return;
        }
        self.pull_modal_open = true;
        if self.pull_phase == PullPhase::Idle {
            self.pull_status = self.t().pull_hint.into();
        }
        cx.notify();
    }

    fn dismiss_pull_modal(&mut self, cx: &mut Context<Self>) {
        if self.pull_phase == PullPhase::Pulling {
            return;
        }
        self.pull_modal_open = false;
        self.pull_phase = PullPhase::Idle;
        self.pull_status = self.t().pull_hint.into();
        self.pull_progress = 0.0;
        self.pull_logs.clear();
        cx.notify();
    }

    fn open_create_modal(
        &mut self,
        prefill_image: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sdk_locked() || self.ops_disabled() {
            return;
        }
        if let Some(image) = prefill_image {
            self.image_input.update(cx, |state, cx| {
                state.set_value(&image, window, cx);
            });
        }
        self.create_modal_open = true;
        cx.notify();
    }

    fn dismiss_create_modal(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.create_modal_open = false;
        cx.notify();
    }

    fn dismiss_logs_modal(&mut self, cx: &mut Context<Self>) {
        self.logs_modal_open = false;
        self.logs_text = SharedString::default();
        self.logs_title = SharedString::default();
        cx.notify();
    }

    fn submit_create_container(&mut self, cx: &mut Context<Self>) {
        if self.sdk_locked() || self.busy || self.pull_phase == PullPhase::Pulling {
            return;
        }
        let t = self.t();
        let image = self.image_input.read(cx).value().trim().to_string();
        if image.is_empty() {
            self.set_status(t.image_empty, cx);
            return;
        }
        let name_raw = self.name_input.read(cx).value().to_string();
        let name = name_raw.trim();
        let name_opt = if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        };
        let ports_raw = self.ports_input.read(cx).value().to_string();
        let mounts_raw = self.bind_mounts_input.read(cx).value().to_string();
        let command_raw = self.command_input.read(cx).value().to_string();
        let command_trim = command_raw.trim();
        let command_opt = if command_trim.is_empty() {
            None
        } else {
            Some(command_trim.to_string())
        };

        let ports = match Self::parse_port_mappings(&ports_raw) {
            Ok(p) => p,
            Err(err) => {
                self.set_status(i18n::fmt1(t.parse_ports_failed, "err", err), cx);
                return;
            }
        };
        let volumes = match Self::parse_bind_mounts(&mounts_raw) {
            Ok(v) => v,
            Err(err) => {
                self.set_status(i18n::fmt1(t.parse_bind_mounts_failed, "err", err), cx);
                return;
            }
        };

        let request = CreateContainerRequest {
            image,
            name: name_opt,
            ports,
            volumes,
            auto_remove: self.create_auto_remove,
            command: command_opt,
        };

        let Some(client) = self.client.clone() else {
            return;
        };
        self.busy = true;
        cx.notify();
        match client.create_container(request) {
            Ok(created) => {
                self.busy = false;
                self.create_modal_open = false;
                self.set_status(i18n::fmt1(t.created_container, "name", &created), cx);
                self.refresh(cx);
            }
            Err(err) => {
                self.busy = false;
                self.set_status(i18n::fmt1(t.error_prefix, "err", err.to_string()), cx);
            }
        }
    }

    fn open_container_logs(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.ops_disabled() {
            return;
        }
        let Some(item) = self.containers.get(index) else {
            return;
        };
        let key = Self::container_key(item);
        let Some(client) = self.client.clone() else {
            return;
        };
        let t = self.t();
        self.busy = true;
        cx.notify();
        match client.container_logs(&key) {
            Ok(text) => {
                self.busy = false;
                self.logs_title = key.into();
                self.logs_text = text.into();
                self.logs_modal_open = true;
            }
            Err(err) => {
                self.busy = false;
                self.set_status(i18n::fmt1(t.container_logs_failed, "err", err.to_string()), cx);
            }
        }
        cx.notify();
    }

    fn t(&self) -> &'static Strings {
        self.locale.strings()
    }

    fn set_status(&mut self, msg: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status_line = msg.into();
        cx.notify();
    }

    fn apply_locale(&mut self, locale: Locale, window: &mut Window, cx: &mut Context<Self>) {
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

    fn apply_theme(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_mode == mode {
            return;
        }
        self.theme_mode = mode;
        Theme::change(mode, Some(window), cx);
        cx.notify();
    }

    fn localize_engine_message(&self, status: &EngineStatus) -> String {
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

    fn refresh(&mut self, cx: &mut Context<Self>) {
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

    fn with_busy(
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

    fn append_sdk_log(&mut self, line: impl Into<SharedString>) {
        self.sdk_setup_logs.push(line.into());
        if self.sdk_setup_logs.len() > MAX_SDK_LOG_LINES {
            let drop = self.sdk_setup_logs.len() - MAX_SDK_LOG_LINES;
            self.sdk_setup_logs.drain(0..drop);
        }
    }

    fn begin_sdk_setup(&mut self, force_redownload: bool, cx: &mut Context<Self>) {
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
    fn poll_sdk_setup(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(rx) = self.sdk_setup_rx.take() else {
            return false;
        };

        let mut events = Vec::new();
        let mut disconnected = false;
        loop {
            match rx.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

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

    fn append_pull_log(&mut self, line: impl Into<SharedString>) {
        self.pull_logs.push(line.into());
        if self.pull_logs.len() > MAX_PULL_LOG_LINES {
            let drop = self.pull_logs.len() - MAX_PULL_LOG_LINES;
            self.pull_logs.drain(0..drop);
        }
    }

    fn begin_pull(&mut self, reference: String, cx: &mut Context<Self>) {
        if self.sdk_locked() || self.pull_phase == PullPhase::Pulling {
            return;
        }
        let reference = reference.trim().to_string();
        if reference.is_empty() {
            self.set_status(self.t().image_ref_empty, cx);
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };

        let (tx, rx) = std::sync::mpsc::channel();
        client.start_pull(&reference, tx);
        self.pull_rx = Some(rx);
        self.pull_phase = PullPhase::Pulling;
        self.pull_progress = 0.0;
        let pulling = i18n::fmt1(self.t().pulling_image, "name", &reference);
        self.pull_status = pulling.clone().into();
        self.pull_logs.clear();
        self.append_pull_log(format!("sdk pull {reference}"));
        self.status_line = pulling.into();
        self.pull_poll_generation = self.pull_poll_generation.wrapping_add(1);
        let generation = self.pull_poll_generation;
        cx.notify();

        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(80)).await;
                let cont = this
                    .update(cx, |this, cx| {
                        if this.pull_poll_generation != generation {
                            return false;
                        }
                        this.poll_pull(cx)
                    })
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        })
        .detach();
    }

    /// Returns `true` while the pull is still active.
    fn poll_pull(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(rx) = self.pull_rx.take() else {
            return false;
        };

        let mut events = Vec::new();
        let mut disconnected = false;
        loop {
            match rx.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        let mut finished = disconnected;
        for event in events {
            match event {
                PullEvent::Status(status) => {
                    self.pull_status = status.into();
                }
                PullEvent::Progress(value) => {
                    self.pull_progress = value.clamp(0.0, 100.0);
                }
                PullEvent::Log(line) => {
                    self.append_pull_log(line);
                }
                PullEvent::Finished(result) => {
                    finished = true;
                    match result {
                        Ok(()) => {
                            self.pull_phase = PullPhase::Success;
                            self.pull_progress = 100.0;
                            self.pull_status = self.t().pull_complete.into();
                            self.append_pull_log(self.t().pull_finished_ok);
                            self.status_line = self.t().pulled_image.into();
                            self.refresh(cx);
                        }
                        Err(err) => {
                            self.pull_phase = PullPhase::Failed;
                            let msg = i18n::fmt1(self.t().pull_failed_msg, "err", &err);
                            self.pull_status = msg.clone().into();
                            self.append_pull_log(format!("✗ {err}"));
                            self.status_line = msg.into();
                        }
                    }
                }
            }
        }

        if disconnected && self.pull_phase == PullPhase::Pulling {
            self.pull_phase = PullPhase::Failed;
            self.pull_status = self.t().pull_channel_disconnected.into();
            self.append_pull_log(self.t().pull_worker_disconnected);
            finished = true;
        }

        if finished {
            self.pull_rx = None;
        } else {
            self.pull_rx = Some(rx);
        }
        cx.notify();
        self.pull_phase == PullPhase::Pulling
    }

    fn selected_container(&self) -> Option<&ContainerInfo> {
        self.selected_container
            .and_then(|i| self.containers.get(i))
    }

    fn selected_image(&self) -> Option<&ImageInfo> {
        self.selected_image.and_then(|i| self.images.get(i))
    }

    fn selected_volume(&self) -> Option<&VolumeInfo> {
        self.selected_volume.and_then(|i| self.volumes.get(i))
    }

    fn render_engine_badge(&self, cx: &Context<Self>) -> impl IntoElement {
        let (label, color) = match self.status.backend {
            BackendKind::Live => ("LIVE", cx.theme().green),
            BackendKind::Unavailable => ("OFF", cx.theme().red),
        };

        h_flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded(cx.theme().radius)
                    .bg(color.opacity(0.18))
                    .text_color(color)
                    .text_xs()
                    .font_semibold()
                    .child(label),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.status.version.clone()),
            )
    }

    fn render_pull_badge(&self, cx: &Context<Self>) -> impl IntoElement {
        let color = match self.pull_phase {
            PullPhase::Idle => cx.theme().muted_foreground,
            PullPhase::Pulling => cx.theme().blue,
            PullPhase::Success => cx.theme().green,
            PullPhase::Failed => cx.theme().red,
        };
        div()
            .px_2()
            .py_0p5()
            .rounded(cx.theme().radius)
            .bg(color.opacity(0.18))
            .text_color(color)
            .text_xs()
            .font_semibold()
            .child(self.pull_phase.label(self.t()))
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        Sidebar::new("nav")
            .w(px(220.))
            .header(
                SidebarHeader::new().child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size_8()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().sidebar_primary)
                                .text_color(cx.theme().sidebar_primary_foreground)
                                .child(Icon::new(IconName::SquareTerminal)),
                        )
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(
                                    div()
                                        .font_semibold()
                                        .text_sm()
                                        .child("WSLc Desktop"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(t.app_subtitle),
                                ),
                        ),
                ),
            )
            .child(
                SidebarGroup::new(t.browse).child(
                    SidebarMenu::new().children([
                        SidebarMenuItem::new(t.containers)
                            .icon(IconName::HardDrive)
                            .active(self.section == Section::Containers)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Containers;
                                cx.notify();
                            })),
                        SidebarMenuItem::new(t.images)
                            .icon(IconName::Folder)
                            .active(self.section == Section::Images)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Images;
                                cx.notify();
                            })),
                        SidebarMenuItem::new(t.volumes)
                            .icon(IconName::Inbox)
                            .active(self.section == Section::Volumes)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Volumes;
                                cx.notify();
                            })),
                    ]),
                ),
            )
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        // TitleBar content is WindowControlArea::Drag (HTCAPTION on Windows),
        // which swallows clicks. Keep TitleBar empty for drag + window controls,
        // and overlay the settings button as a sibling that `.occlude()`s the
        // drag hitbox underneath.
        div()
            .relative()
            .w_full()
            .child(TitleBar::new())
            .child(
                h_flex()
                    .id("title-bar-settings")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    // Leave room for Windows min/max/close (~3 × 35px).
                    .right(px(120.))
                    .items_center()
                    .occlude()
                    .child(
                        Button::new("titlebar-settings")
                            .ghost()
                            .icon(IconName::Settings)
                            .tooltip(t.settings)
                            .selected(self.section == Section::Settings)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    this.section = Section::Settings;
                                    cx.notify();
                                }),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Settings;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        h_flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .child(self.section.label(t)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.status.message.clone()),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(self.render_engine_badge(cx))
                    .when(self.section != Section::Settings, |this| {
                        this.child(
                            Button::new("refresh")
                                .label(if self.busy { t.working } else { t.refresh })
                                .icon(IconName::RefreshCw)
                                .disabled(self.ops_disabled())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_status(this.t().refreshing, cx);
                                    this.refresh(cx);
                                })),
                        )
                    }),
            )
    }

    fn render_container_row(
        &self,
        index: usize,
        item: &ContainerInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = self.t();
        let selected = self.selected_container == Some(index);
        let running = item.is_running();
        let display_name = if item.name.is_empty() {
            item.id.chars().take(12).collect::<String>()
        } else {
            item.name.clone()
        };
        let id_short = item.id.chars().take(12).collect::<String>();
        let image = item.image.clone();
        let status = item.status.clone();
        let ports = if item.ports.is_empty() {
            "—".to_string()
        } else {
            item.ports.clone()
        };
        let volumes = if item.volumes.is_empty() {
            "—".to_string()
        } else {
            item.volumes.clone()
        };
        let ports_label = t.ports_label;
        let mounts_label = t.bind_mounts_label;
        let ops_off = self.ops_disabled();

        div()
            .id(("container-row", index))
            .w_full()
            .px_3()
            .py_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if selected {
                cx.theme().border
            } else {
                cx.theme().transparent
            })
            .bg(if selected {
                cx.theme().accent.opacity(0.12)
            } else {
                cx.theme().transparent
            })
            .hover(|s| s.bg(cx.theme().accent.opacity(0.08)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected_container = Some(index);
                cx.notify();
            }))
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .mt_1()
                            .size_2()
                            .rounded_full()
                            .bg(if running {
                                cx.theme().green
                            } else {
                                cx.theme().muted_foreground.opacity(0.5)
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .flex_wrap()
                                    .child(div().font_medium().text_sm().child(display_name))
                                    .when(!item.name.is_empty(), |row| {
                                        row.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(id_short),
                                        )
                                    })
                                    .when(item.auto_remove, |row| {
                                        row.child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded(cx.theme().radius)
                                                .bg(cx.theme().yellow.opacity(0.2))
                                                .text_color(cx.theme().yellow)
                                                .text_xs()
                                                .font_semibold()
                                                .child(t.auto_remove_badge),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(image),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(status),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(format!("{ports_label}: {ports}")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(format!("{mounts_label}: {volumes}")),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .flex_shrink_0()
                            .occlude()
                            .child(
                                Button::new(("container-start", index))
                                    .small()
                                    .label(t.start)
                                    .disabled(ops_off || running)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.selected_container = Some(index);
                                        let key = this
                                            .containers
                                            .get(index)
                                            .map(Self::container_key)
                                            .unwrap_or_default();
                                        let template = this.t().started;
                                        this.with_busy(cx, move |client| {
                                            client.start_container(&key)?;
                                            Ok(i18n::fmt1(template, "name", &key))
                                        });
                                    })),
                            )
                            .child(
                                Button::new(("container-stop", index))
                                    .small()
                                    .label(t.stop)
                                    .disabled(ops_off || !running)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.selected_container = Some(index);
                                        let key = this
                                            .containers
                                            .get(index)
                                            .map(Self::container_key)
                                            .unwrap_or_default();
                                        let template = this.t().stopped;
                                        this.with_busy(cx, move |client| {
                                            client.stop_container(&key)?;
                                            Ok(i18n::fmt1(template, "name", &key))
                                        });
                                    })),
                            )
                            .child(
                                Button::new(("container-logs", index))
                                    .small()
                                    .label(t.logs)
                                    .disabled(ops_off)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_container_logs(index, cx);
                                    })),
                            )
                            .child(
                                Button::new(("container-remove", index))
                                    .small()
                                    .danger()
                                    .label(t.remove)
                                    .disabled(ops_off)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.selected_container = Some(index);
                                        let key = this
                                            .containers
                                            .get(index)
                                            .map(Self::container_key)
                                            .unwrap_or_default();
                                        let template = this.t().removed;
                                        this.with_busy(cx, move |client| {
                                            client.remove_container(&key)?;
                                            Ok(i18n::fmt1(template, "name", &key))
                                        });
                                    })),
                            ),
                    ),
            )
    }

    fn render_containers(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let running_count = self.containers.iter().filter(|c| c.is_running()).count();

        v_flex()
            .size_full()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(i18n::fmt2(
                                t.containers_count,
                                "total",
                                self.containers.len().to_string(),
                                "running",
                                running_count.to_string(),
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("create-container")
                                    .small()
                                    .primary()
                                    .label(t.create)
                                    .icon(IconName::Plus)
                                    .disabled(self.ops_disabled())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_create_modal(None, window, cx);
                                    })),
                            )
                            .child(
                                Button::new("toggle-all")
                                    .small()
                                    .label(if self.show_all_containers {
                                        t.all
                                    } else {
                                        t.running
                                    })
                                    .disabled(self.ops_disabled())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_all_containers = !this.show_all_containers;
                                        this.refresh(cx);
                                    })),
                            )
                            .child(
                                Button::new("start-selected")
                                    .small()
                                    .primary()
                                    .label(t.start)
                                    .disabled(
                                        self.ops_disabled()
                                            || self
                                                .selected_container()
                                                .is_none_or(|c| c.is_running()),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let Some(c) = this.selected_container().cloned() else {
                                            return;
                                        };
                                        let key = if c.name.is_empty() {
                                            c.id.clone()
                                        } else {
                                            c.name.clone()
                                        };
                                        let template = this.t().started;
                                        this.with_busy(cx, move |client| {
                                            client.start_container(&key)?;
                                            Ok(i18n::fmt1(template, "name", &key))
                                        });
                                    })),
                            )
                            .child(
                                Button::new("stop-selected")
                                    .small()
                                    .label(t.stop)
                                    .disabled(
                                        self.ops_disabled()
                                            || self
                                                .selected_container()
                                                .is_none_or(|c| !c.is_running()),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let Some(c) = this.selected_container().cloned() else {
                                            return;
                                        };
                                        let key = if c.name.is_empty() {
                                            c.id.clone()
                                        } else {
                                            c.name.clone()
                                        };
                                        let template = this.t().stopped;
                                        this.with_busy(cx, move |client| {
                                            client.stop_container(&key)?;
                                            Ok(i18n::fmt1(template, "name", &key))
                                        });
                                    })),
                            )
                            .child(
                                Button::new("remove-selected")
                                    .small()
                                    .danger()
                                    .label(t.remove)
                                    .disabled(
                                        self.ops_disabled() || self.selected_container().is_none(),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let Some(c) = this.selected_container().cloned() else {
                                            return;
                                        };
                                        let key = if c.name.is_empty() {
                                            c.id.clone()
                                        } else {
                                            c.name.clone()
                                        };
                                        let template = this.t().removed;
                                        this.with_busy(cx, move |client| {
                                            client.remove_container(&key)?;
                                            Ok(i18n::fmt1(template, "name", &key))
                                        });
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .id("container-list")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .child({
                        if self.containers.is_empty() {
                            v_flex().gap_1().w_full().child(self.empty_state(
                                t.no_containers_title,
                                t.no_containers_detail,
                                cx,
                            ))
                        } else {
                            let rows: Vec<_> = self
                                .containers
                                .iter()
                                .enumerate()
                                .map(|(index, item)| {
                                    self.render_container_row(index, item, cx).into_any_element()
                                })
                                .collect();
                            v_flex().gap_1().w_full().children(rows)
                        }
                    }),
            )
    }

    fn render_image_row(
        &self,
        index: usize,
        item: &ImageInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.selected_image == Some(index);
        let reference = item.reference();
        let id_short = item.id.chars().take(16).collect::<String>();
        let meta = format!("{} · {}", item.size, item.created);

        div()
            .id(("image-row", index))
            .w_full()
            .px_3()
            .py_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if selected {
                cx.theme().border
            } else {
                cx.theme().transparent
            })
            .bg(if selected {
                cx.theme().accent.opacity(0.12)
            } else {
                cx.theme().transparent
            })
            .hover(|s| s.bg(cx.theme().accent.opacity(0.08)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected_image = Some(index);
                cx.notify();
            }))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(Icon::new(IconName::Folder).text_color(cx.theme().muted_foreground))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().font_medium().text_sm().child(reference))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(format!("{id_short} · {meta}")),
                            ),
                    ),
            )
    }

    fn render_pull_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let pulling = self.pull_phase == PullPhase::Pulling;
        let can_close = !pulling;
        let log_lines: Vec<_> = self
            .pull_logs
            .iter()
            .enumerate()
            .map(|(i, line)| {
                div()
                    .id(("pull-log", i))
                    .text_xs()
                    .font_family("monospace")
                    .text_color(cx.theme().muted_foreground)
                    .child(line.clone())
                    .into_any_element()
            })
            .collect();

        div()
            .id("pull-modal")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().overlay)
            .occlude()
            .child(
                v_flex()
                    .w(px(560.))
                    .max_h(px(520.))
                    .m_6()
                    .p_5()
                    .gap_3()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .shadow_md()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(div().font_semibold().text_base().child(t.pull_image))
                            .child(self.render_pull_badge(cx)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().min_w_0().child(Input::new(&self.pull_input)))
                            .child(
                                Button::new("pull-image")
                                    .primary()
                                    .label(if pulling { t.pulling } else { t.pull })
                                    .icon(IconName::ArrowDown)
                                    .disabled(self.sdk_locked() || pulling)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let reference = this.pull_input.read(cx).value().to_string();
                                        this.begin_pull(reference, cx);
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .truncate()
                                            .child(self.pull_status.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("{:.0}%", self.pull_progress)),
                                    ),
                            )
                            .child(
                                Progress::new("pull-progress")
                                    .w_full()
                                    .h_2()
                                    .value(self.pull_progress),
                            ),
                    )
                    .child(
                        div()
                            .id("pull-log")
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
                                    if log_lines.is_empty() {
                                        list.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(t.pull_logs_placeholder),
                                        )
                                    } else {
                                        list.children(log_lines)
                                    }
                                }),
                            ),
                    )
                    .when(can_close, |el| {
                        el.child(
                            h_flex()
                                .justify_end()
                                .child(
                                    Button::new("pull-modal-close")
                                        .label(t.close)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.dismiss_pull_modal(cx);
                                        })),
                                ),
                        )
                    }),
            )
    }

    fn render_images(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        v_flex()
            .size_full()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(i18n::fmt1(
                                t.images_count,
                                "count",
                                self.images.len().to_string(),
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("open-pull-modal")
                                    .small()
                                    .primary()
                                    .label(t.pull)
                                    .icon(IconName::ArrowDown)
                                    .disabled(self.ops_disabled())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_pull_modal(cx);
                                    })),
                            )
                            .child(
                                Button::new("create-from-image")
                                    .small()
                                    .label(t.create)
                                    .disabled(
                                        self.ops_disabled() || self.selected_image().is_none(),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let Some(img) = this.selected_image().cloned() else {
                                            return;
                                        };
                                        let reference = img.reference();
                                        this.open_create_modal(Some(reference), window, cx);
                                    })),
                            )
                            .child(
                                Button::new("remove-image")
                                    .small()
                                    .danger()
                                    .label(t.remove)
                                    .disabled(
                                        self.ops_disabled() || self.selected_image().is_none(),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let Some(img) = this.selected_image().cloned() else {
                                            return;
                                        };
                                        let reference = img.reference();
                                        let template = this.t().removed;
                                        this.with_busy(cx, move |client| {
                                            client.remove_image(&reference)?;
                                            Ok(i18n::fmt1(template, "name", &reference))
                                        });
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .id("image-list")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .child({
                        if self.images.is_empty() {
                            v_flex().gap_1().w_full().child(self.empty_state(
                                t.no_images_title,
                                t.no_images_detail,
                                cx,
                            ))
                        } else {
                            let rows: Vec<_> = self
                                .images
                                .iter()
                                .enumerate()
                                .map(|(index, item)| {
                                    self.render_image_row(index, item, cx).into_any_element()
                                })
                                .collect();
                            v_flex().gap_1().w_full().children(rows)
                        }
                    }),
            )
    }

    fn render_volume_row(
        &self,
        index: usize,
        item: &VolumeInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.selected_volume == Some(index);
        let name = item.name.clone();
        let driver = item.driver.clone();
        let mount = if item.mountpoint.is_empty() {
            "—".to_string()
        } else {
            item.mountpoint.clone()
        };
        let meta = format!(
            "{} · {} · {}",
            driver,
            if item.size.is_empty() {
                "—"
            } else {
                item.size.as_str()
            },
            if item.created.is_empty() {
                "—"
            } else {
                item.created.as_str()
            }
        );

        div()
            .id(("volume-row", index))
            .w_full()
            .px_3()
            .py_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if selected {
                cx.theme().border
            } else {
                cx.theme().transparent
            })
            .bg(if selected {
                cx.theme().accent.opacity(0.12)
            } else {
                cx.theme().transparent
            })
            .hover(|s| s.bg(cx.theme().accent.opacity(0.08)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected_volume = Some(index);
                cx.notify();
            }))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::HardDrive).text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().font_medium().text_sm().child(name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(format!("{meta} · {mount}")),
                            ),
                    ),
            )
    }

    fn render_volumes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        v_flex()
            .size_full()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.volume_input)))
                    .child(
                        Button::new("create-volume")
                            .primary()
                            .label(t.create)
                            .icon(IconName::Plus)
                            .disabled(self.ops_disabled())
                            .on_click(cx.listener(|this, _, _, cx| {
                                let name = this.volume_input.read(cx).value().to_string();
                                let template = this.t().created_volume;
                                this.with_busy(cx, move |client| {
                                    client.create_volume(&name)?;
                                    Ok(i18n::fmt1(template, "name", &name))
                                });
                            })),
                    )
                    .child(
                        Button::new("remove-volume")
                            .danger()
                            .label(t.remove)
                            .disabled(
                                self.ops_disabled() || self.selected_volume().is_none(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                let Some(vol) = this.selected_volume().cloned() else {
                                    return;
                                };
                                let name = vol.name;
                                let template = this.t().removed;
                                this.with_busy(cx, move |client| {
                                    client.remove_volume(&name)?;
                                    Ok(i18n::fmt1(template, "name", &name))
                                });
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(i18n::fmt1(
                        t.volumes_count,
                        "count",
                        self.volumes.len().to_string(),
                    )),
            )
            .child(
                div()
                    .id("volume-list")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .child({
                        if self.volumes.is_empty() {
                            v_flex().gap_1().w_full().child(self.empty_state(
                                t.no_volumes_title,
                                t.no_volumes_detail,
                                cx,
                            ))
                        } else {
                            let rows: Vec<_> = self
                                .volumes
                                .iter()
                                .enumerate()
                                .map(|(index, item)| {
                                    self.render_volume_row(index, item, cx).into_any_element()
                                })
                                .collect();
                            v_flex().gap_1().w_full().children(rows)
                        }
                    }),
            )
    }

    fn render_create_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let creating = self.busy && self.create_modal_open;
        let auto_remove = self.create_auto_remove;

        div()
            .id("create-container-modal")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().overlay)
            .occlude()
            .child(
                v_flex()
                    .w(px(560.))
                    .max_h(px(640.))
                    .m_6()
                    .p_5()
                    .gap_3()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .shadow_md()
                    .child(div().font_semibold().text_base().child(t.create_container_title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.create_container_intro),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new(t.image_label))
                            .child(Input::new(&self.image_input)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new(t.name_optional))
                            .child(Input::new(&self.name_input)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new(t.ports_label))
                            .child(Input::new(&self.ports_input)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new(t.bind_mounts_label))
                            .child(Input::new(&self.bind_mounts_input)),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(Label::new(t.auto_remove_label))
                            .child(
                                Button::new("create-auto-remove")
                                    .small()
                                    .label(if auto_remove {
                                        t.auto_remove_on
                                    } else {
                                        t.auto_remove_off
                                    })
                                    .when(auto_remove, |b| b.primary())
                                    .disabled(creating)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.create_auto_remove = !this.create_auto_remove;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(Label::new(t.command_label))
                            .child(Input::new(&self.command_input)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("create-cancel")
                                    .label(t.close)
                                    .disabled(creating)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.dismiss_create_modal(cx);
                                    })),
                            )
                            .child(
                                Button::new("create-submit")
                                    .primary()
                                    .label(if creating { t.working } else { t.create })
                                    .icon(IconName::Plus)
                                    .disabled(self.sdk_locked() || creating || self.pull_phase == PullPhase::Pulling)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.submit_create_container(cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_logs_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let body: SharedString = if self.logs_text.is_empty() {
            t.logs_empty.into()
        } else {
            self.logs_text.clone()
        };

        div()
            .id("logs-modal")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().overlay)
            .occlude()
            .child(
                v_flex()
                    .w(px(640.))
                    .max_h(px(520.))
                    .m_6()
                    .p_5()
                    .gap_3()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .shadow_md()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(div().font_semibold().text_base().child(t.container_logs_title))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(self.logs_title.clone()),
                            ),
                    )
                    .child(
                        div()
                            .id("container-logs-body")
                            .h(px(320.))
                            .w_full()
                            .overflow_y_scroll()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background.opacity(0.65))
                            .p_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(cx.theme().foreground)
                                    .child(body),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_end()
                            .child(
                                Button::new("logs-modal-close")
                                    .label(t.close)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.dismiss_logs_modal(cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let locale = self.locale;
        let theme_mode = self.theme_mode;

        v_flex()
            .size_full()
            .gap_6()
            .max_w(px(560.))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t.settings_intro),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary.opacity(0.28))
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
                                    .label(Locale::Zh.label())
                                    .when(locale == Locale::Zh, |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_locale(Locale::Zh, window, cx);
                                    })),
                            )
                            .child(
                                Button::new("lang-en")
                                    .label(Locale::En.label())
                                    .when(locale == Locale::En, |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_locale(Locale::En, window, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary.opacity(0.28))
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
                                    .label(t.theme_dark)
                                    .when(theme_mode == ThemeMode::Dark, |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_theme(ThemeMode::Dark, window, cx);
                                    })),
                            )
                            .child(
                                Button::new("theme-light")
                                    .label(t.theme_light)
                                    .when(theme_mode == ThemeMode::Light, |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_theme(ThemeMode::Light, window, cx);
                                    })),
                            ),
                    ),
            )
    }

    fn empty_state(
        &self,
        title: &'static str,
        detail: &'static str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .items_center()
            .justify_center()
            .gap_2()
            .py_16()
            .child(
                div()
                    .text_sm()
                    .font_medium()
                    .text_color(cx.theme().foreground)
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .max_w(px(360.))
                    .text_center()
                    .child(detail),
            )
    }

    fn render_status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        h_flex()
            .h_8()
            .px_3()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tab_bar)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(self.status_line.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(i18n::fmt1(
                        self.t().binary_label,
                        "name",
                        &self.status.binary,
                    )),
            )
    }

    fn render_sdk_setup_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

        div()
            .id("sdk-setup-modal")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().overlay)
            .occlude()
            .child(
                v_flex()
                    .w(px(560.))
                    .max_h(px(520.))
                    .m_6()
                    .p_5()
                    .gap_3()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .shadow_md()
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

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_setup = self.sdk_setup_phase.is_some();
        div()
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .size_full()
                    .child(self.render_sidebar(cx))
                    .child(
                        v_flex()
                            .size_full()
                            .min_w_0()
                            .child(self.render_title_bar(cx))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_h_0()
                                    .p_4()
                                    .gap_4()
                                    .child(self.render_toolbar(cx))
                                    .child(
                                        div().id("section").flex_1().min_h_0().map(|panel| {
                                            match self.section {
                                                Section::Containers => {
                                                    panel.child(self.render_containers(cx))
                                                }
                                                Section::Images => {
                                                    panel.child(self.render_images(cx))
                                                }
                                                Section::Volumes => {
                                                    panel.child(self.render_volumes(cx))
                                                }
                                                Section::Settings => {
                                                    panel.child(self.render_settings(cx))
                                                }
                                            }
                                        }),
                                    ),
                            )
                            .child(self.render_status_bar(cx)),
                    ),
            )
            .when(show_setup, |root| {
                root.child(self.render_sdk_setup_modal(cx))
            })
            .when(self.pull_modal_open, |root| root.child(self.render_pull_modal(cx)))
            .when(self.create_modal_open, |root| {
                root.child(self.render_create_modal(cx))
            })
            .when(self.logs_modal_open, |root| root.child(self.render_logs_modal(cx)))
    }
}
