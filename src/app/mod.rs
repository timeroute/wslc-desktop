use std::sync::mpsc::Receiver;

use gpui_kit::component::{
    ActiveTheme, ThemeMode,
    h_flex,
    input::InputState,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::i18n::Locale;
use crate::wslc::{
    ContainerInfo, EngineStatus, ImageInfo, PullEvent, SdkSetupEvent, VolumeInfo, WslcClient,
};

mod containers;
mod images;
mod parsing;
mod poll;
mod refresh;
mod sdk_setup;
mod settings;
mod shell;
mod state;
mod volumes;
mod widgets;

pub use state::Section;

use state::{PullPhase, SdkSetupPhase};

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
            busy_rx: None,
            busy_poll_generation: 0,
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
            .when(self.busy && !show_setup && !self.pull_modal_open, |root| {
                root.child(self.render_busy_overlay(cx))
            })
    }
}
