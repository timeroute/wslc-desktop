use std::sync::mpsc::Receiver;

use gpui_kit::component::{ActiveTheme, ThemeMode, h_flex, input::InputState, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::i18n::Locale;
use crate::wslc::{
    ContainerInfo, EngineStatus, ImageInfo, PullEvent, SdkSetupEvent, SessionPrefs, SetupStepState,
    TemplateStore, VolumeInfo, WslcClient,
};

mod close;
mod confirm;
mod containers;
mod images;
mod overview;
mod parsing;
mod poll;
mod refresh;
mod sdk_setup;
mod settings;
mod shell;
mod state;
pub(crate) mod tray_icon;
mod volumes;
mod widgets;

pub use state::Section;

use state::{ConfirmAction, PullPhase, SdkSetupPhase, UpdateCheckPhase};

/// Shared handles so tray actions can reach the main window / view.
pub struct DesktopShell {
    pub window: AnyWindowHandle,
    pub view: WeakEntity<AppView>,
}

impl Global for DesktopShell {}

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
    /// Background result for `with_busy` (start/stop/create/…).
    busy_rx: Option<Receiver<anyhow::Result<String>>>,
    busy_poll_generation: u64,
    status_line: SharedString,
    image_input: Entity<InputState>,
    name_input: Entity<InputState>,
    ports_input: Entity<InputState>,
    bind_mounts_input: Entity<InputState>,
    named_volumes_input: Entity<InputState>,
    command_input: Entity<InputState>,
    template_name_input: Entity<InputState>,
    template_rename_input: Entity<InputState>,
    pull_input: Entity<InputState>,
    volume_input: Entity<InputState>,
    exec_input: Entity<InputState>,
    create_auto_remove: bool,
    templates: TemplateStore,
    template_save_open: bool,
    template_renaming: Option<String>,
    template_banner_error: bool,
    template_banner: Option<SharedString>,
    volume_size_gib: u32,
    session_prefs: SessionPrefs,
    pull_modal_open: bool,
    create_modal_open: bool,
    volume_modal_open: bool,
    logs_modal_open: bool,
    logs_title: SharedString,
    logs_text: SharedString,
    exec_modal_open: bool,
    exec_target_key: SharedString,
    confirm: Option<ConfirmAction>,
    close_prompt_open: bool,
    window_hidden: bool,
    allow_window_close: bool,
    pending_quit_after_stop: bool,
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
    wsl_step: SetupStepState,
    sdk_step: SetupStepState,
    sdk_logs_open: bool,
    sdk_logs_copied: bool,
    sdk_log_error: Option<SharedString>,
    update_check: UpdateCheckPhase,
    update_check_poll_generation: u64,
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
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder(t.name_placeholder));
        let ports_input = cx.new(|cx| InputState::new(window, cx).placeholder(t.ports_placeholder));
        let bind_mounts_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.bind_mounts_placeholder));
        let named_volumes_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.named_volumes_placeholder));
        let command_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.command_placeholder));
        let template_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.template_name_placeholder));
        let template_rename_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.template_name_placeholder));
        let pull_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("library/alpine:latest")
                .default_value("alpine:latest")
        });
        let volume_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.volume_placeholder));
        let exec_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t.container_exec_placeholder)
                .default_value("uname -a")
        });
        let session_prefs = WslcClient::load_session_prefs();

        let mut view = Self {
            client: None,
            status: EngineStatus::unavailable(t.sdk_locked),
            section: Section::Overview,
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
            busy_rx: None,
            busy_poll_generation: 0,
            status_line: t.sdk_locked.into(),
            image_input,
            name_input,
            ports_input,
            bind_mounts_input,
            named_volumes_input,
            command_input,
            template_name_input,
            template_rename_input,
            pull_input,
            volume_input,
            exec_input,
            create_auto_remove: false,
            templates: TemplateStore::load(),
            template_save_open: false,
            template_renaming: None,
            template_banner_error: false,
            template_banner: None,
            volume_size_gib: 1,
            session_prefs,
            pull_modal_open: false,
            create_modal_open: false,
            volume_modal_open: false,
            logs_modal_open: false,
            logs_title: SharedString::default(),
            logs_text: SharedString::default(),
            exec_modal_open: false,
            exec_target_key: SharedString::default(),
            confirm: None,
            close_prompt_open: false,
            window_hidden: false,
            allow_window_close: false,
            pending_quit_after_stop: false,
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
            wsl_step: SetupStepState::Pending,
            sdk_step: SetupStepState::Pending,
            sdk_logs_open: false,
            sdk_logs_copied: false,
            sdk_log_error: None,
            update_check: UpdateCheckPhase::Idle,
            update_check_poll_generation: 0,
        };
        view.begin_sdk_setup(false, cx);
        view
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
                h_flex().size_full().child(self.render_sidebar(cx)).child(
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
                                .child(div().id("section").flex_1().min_h_0().map(|panel| {
                                    match self.section {
                                        Section::Overview => panel.child(self.render_overview(cx)),
                                        Section::Containers => {
                                            panel.child(self.render_containers(cx))
                                        }
                                        Section::Images => panel.child(self.render_images(cx)),
                                        Section::Volumes => panel.child(self.render_volumes(cx)),
                                        Section::Settings => panel.child(self.render_settings(cx)),
                                    }
                                })),
                        )
                        .child(self.render_status_bar(cx)),
                ),
            )
            .when(show_setup, |root| {
                root.child(self.render_sdk_setup_modal(cx))
            })
            .when(self.pull_modal_open, |root| {
                root.child(self.render_pull_modal(cx))
            })
            .when(self.create_modal_open, |root| {
                root.child(self.render_create_modal(cx))
            })
            .when(self.volume_modal_open, |root| {
                root.child(self.render_volume_modal(cx))
            })
            .when(self.logs_modal_open, |root| {
                root.child(self.render_logs_modal(cx))
            })
            .when(self.exec_modal_open, |root| {
                root.child(self.render_exec_modal(cx))
            })
            .when(self.confirm.is_some(), |root| {
                root.child(self.render_confirm_modal(cx))
            })
            .when(self.close_prompt_open, |root| {
                root.child(self.render_close_prompt_modal(cx))
            })
            .when(
                self.busy
                    && !show_setup
                    && !self.pull_modal_open
                    && !self.volume_modal_open
                    && !self.exec_modal_open
                    && self.confirm.is_none()
                    && !self.close_prompt_open,
                |root| root.child(self.render_busy_overlay(cx)),
            )
    }
}
