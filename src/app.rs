use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    label::Label,
    progress::Progress,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use smol::Timer;

use crate::wslc::{
    BackendKind, ContainerInfo, EngineStatus, ImageInfo, PullEvent, VolumeInfo, WslcClient,
};

const MAX_PULL_LOG_LINES: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Containers = 0,
    Images = 1,
    Volumes = 2,
    Run = 3,
}

impl Section {
    fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Containers,
            1 => Self::Images,
            2 => Self::Volumes,
            3 => Self::Run,
            _ => Self::Containers,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Containers => "Containers",
            Self::Images => "Images",
            Self::Volumes => "Volumes",
            Self::Run => "Run",
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
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Pulling => "PULLING",
            Self::Success => "OK",
            Self::Failed => "FAILED",
        }
    }
}

pub struct AppView {
    client: WslcClient,
    status: EngineStatus,
    section: Section,
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
    pull_input: Entity<InputState>,
    volume_input: Entity<InputState>,
    pull_phase: PullPhase,
    pull_status: SharedString,
    pull_progress: f32,
    pull_logs: Vec<SharedString>,
    pull_rx: Option<Receiver<PullEvent>>,
    pull_poll_generation: u64,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let client = WslcClient::detect();
        let status = client.status();
        let image_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("nginx:alpine")
                .default_value("nginx:alpine")
        });
        let name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("optional container name"));
        let pull_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("library/alpine:latest")
                .default_value("alpine:latest")
        });
        let volume_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("my-volume"));

        let mut view = Self {
            client,
            status,
            section: Section::Containers,
            containers: Vec::new(),
            images: Vec::new(),
            volumes: Vec::new(),
            selected_container: None,
            selected_image: None,
            selected_volume: None,
            show_all_containers: true,
            busy: false,
            status_line: "Ready".into(),
            image_input,
            name_input,
            pull_input,
            volume_input,
            pull_phase: PullPhase::Idle,
            pull_status: "Enter an image reference and pull".into(),
            pull_progress: 0.0,
            pull_logs: Vec::new(),
            pull_rx: None,
            pull_poll_generation: 0,
        };
        view.refresh(cx);
        view
    }

    fn set_status(&mut self, msg: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status_line = msg.into();
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.status = self.client.status();
        match self.client.list_containers(self.show_all_containers) {
            Ok(rows) => {
                self.containers = rows;
                if self
                    .selected_container
                    .is_some_and(|i| i >= self.containers.len())
                {
                    self.selected_container = None;
                }
            }
            Err(err) => self.status_line = format!("List containers failed: {err}").into(),
        }
        match self.client.list_images() {
            Ok(rows) => {
                self.images = rows;
                if self.selected_image.is_some_and(|i| i >= self.images.len()) {
                    self.selected_image = None;
                }
            }
            Err(err) => self.status_line = format!("List images failed: {err}").into(),
        }
        match self.client.list_volumes() {
            Ok(rows) => {
                self.volumes = rows;
                if self.selected_volume.is_some_and(|i| i >= self.volumes.len()) {
                    self.selected_volume = None;
                }
            }
            Err(err) => self.status_line = format!("List volumes failed: {err}").into(),
        }
        if self.status_line.as_ref() == "Ready"
            || self.status_line.as_ref().starts_with("Refreshed")
            || self.status_line.as_ref().starts_with("Started")
            || self.status_line.as_ref().starts_with("Stopped")
            || self.status_line.as_ref().starts_with("Removed")
            || self.status_line.as_ref().starts_with("Launched")
            || self.status_line.as_ref().starts_with("Pulled")
            || self.status_line.as_ref().starts_with("Created volume")
        {
            let mode = match self.status.backend {
                BackendKind::Live => "live",
                BackendKind::Demo => "demo",
            };
            self.status_line = format!(
                "Refreshed · {} containers · {} images · {} volumes · {mode}",
                self.containers.len(),
                self.images.len(),
                self.volumes.len()
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
        if self.busy || self.pull_phase == PullPhase::Pulling {
            return;
        }
        self.busy = true;
        cx.notify();
        match op(&self.client) {
            Ok(msg) => {
                self.busy = false;
                self.set_status(msg, cx);
                self.refresh(cx);
            }
            Err(err) => {
                self.busy = false;
                self.set_status(format!("Error: {err}"), cx);
            }
        }
    }

    fn append_pull_log(&mut self, line: impl Into<SharedString>) {
        self.pull_logs.push(line.into());
        if self.pull_logs.len() > MAX_PULL_LOG_LINES {
            let drop = self.pull_logs.len() - MAX_PULL_LOG_LINES;
            self.pull_logs.drain(0..drop);
        }
    }

    fn begin_pull(&mut self, reference: String, cx: &mut Context<Self>) {
        if self.pull_phase == PullPhase::Pulling {
            return;
        }
        let reference = reference.trim().to_string();
        if reference.is_empty() {
            self.set_status("Image reference is empty", cx);
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        self.client.start_pull(&reference, tx);
        self.pull_rx = Some(rx);
        self.pull_phase = PullPhase::Pulling;
        self.pull_progress = 0.0;
        self.pull_status = format!("Pulling {reference}…").into();
        self.pull_logs.clear();
        self.append_pull_log(format!("$ wslc pull {reference}"));
        self.status_line = format!("Pulling {reference}…").into();
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
                            self.pull_status = "Pull complete".into();
                            self.append_pull_log("✓ Pull finished successfully");
                            self.status_line = "Pulled image".into();
                            self.refresh(cx);
                        }
                        Err(err) => {
                            self.pull_phase = PullPhase::Failed;
                            self.pull_status = format!("Pull failed: {err}").into();
                            self.append_pull_log(format!("✗ {err}"));
                            self.status_line = format!("Pull failed: {err}").into();
                        }
                    }
                }
            }
        }

        if disconnected && self.pull_phase == PullPhase::Pulling {
            self.pull_phase = PullPhase::Failed;
            self.pull_status = "Pull channel disconnected".into();
            self.append_pull_log("✗ pull worker disconnected");
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
            BackendKind::Demo => ("DEMO", cx.theme().yellow),
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
            .child(self.pull_phase.label())
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                                        .child("WSL Containers"),
                                ),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("Browse").child(
                    SidebarMenu::new().children([
                        SidebarMenuItem::new("Containers")
                            .icon(IconName::HardDrive)
                            .active(self.section == Section::Containers)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Containers;
                                cx.notify();
                            })),
                        SidebarMenuItem::new("Images")
                            .icon(IconName::Folder)
                            .active(self.section == Section::Images)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Images;
                                cx.notify();
                            })),
                        SidebarMenuItem::new("Volumes")
                            .icon(IconName::Inbox)
                            .active(self.section == Section::Volumes)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Volumes;
                                cx.notify();
                            })),
                        SidebarMenuItem::new("Run")
                            .icon(IconName::Play)
                            .active(self.section == Section::Run)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Run;
                                cx.notify();
                            })),
                    ]),
                ),
            )
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .child(self.section.label()),
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
                    .child(
                        Button::new("refresh")
                            .label(if self.busy { "Working…" } else { "Refresh" })
                            .icon(IconName::RefreshCw)
                            .disabled(self.busy || self.pull_phase == PullPhase::Pulling)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_status("Refreshing…", cx);
                                this.refresh(cx);
                            })),
                    ),
            )
    }

    fn render_container_row(
        &self,
        index: usize,
        item: &ContainerInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.selected_container == Some(index);
        let running = item.is_running();
        let name = item.name.clone();
        let image = item.image.clone();
        let status = item.status.clone();
        let ports = if item.ports.is_empty() {
            "—".to_string()
        } else {
            item.ports.clone()
        };
        let id_short = item.id.chars().take(12).collect::<String>();

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
                    .items_center()
                    .gap_3()
                    .child(
                        div()
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
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_baseline()
                                    .child(div().font_medium().text_sm().child(name))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(id_short),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(format!("{image} · {status} · {ports}")),
                            ),
                    ),
            )
    }

    fn render_containers(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .child(format!(
                                "{} total · {} running",
                                self.containers.len(),
                                running_count
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("toggle-all")
                                    .small()
                                    .label(if self.show_all_containers {
                                        "All"
                                    } else {
                                        "Running"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_all_containers = !this.show_all_containers;
                                        this.refresh(cx);
                                    })),
                            )
                            .child(
                                Button::new("start-selected")
                                    .small()
                                    .primary()
                                    .label("Start")
                                    .disabled(
                                        self.busy
                                            || self.pull_phase == PullPhase::Pulling
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
                                        this.with_busy(cx, move |client| {
                                            client.start_container(&key)?;
                                            Ok(format!("Started {key}"))
                                        });
                                    })),
                            )
                            .child(
                                Button::new("stop-selected")
                                    .small()
                                    .label("Stop")
                                    .disabled(
                                        self.busy
                                            || self.pull_phase == PullPhase::Pulling
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
                                        this.with_busy(cx, move |client| {
                                            client.stop_container(&key)?;
                                            Ok(format!("Stopped {key}"))
                                        });
                                    })),
                            )
                            .child(
                                Button::new("remove-selected")
                                    .small()
                                    .danger()
                                    .label("Remove")
                                    .disabled(
                                        self.busy
                                            || self.pull_phase == PullPhase::Pulling
                                            || self.selected_container().is_none(),
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
                                        this.with_busy(cx, move |client| {
                                            client.remove_container(&key)?;
                                            Ok(format!("Removed {key}"))
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
                                "No containers",
                                "Run an image from the Run tab, or refresh after starting one with wslc.",
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

    fn render_pull_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pulling = self.pull_phase == PullPhase::Pulling;
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

        v_flex()
            .gap_3()
            .p_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary.opacity(0.28))
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_medium().text_sm().child("Pull image"))
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
                            .label(if pulling { "Pulling…" } else { "Pull" })
                            .icon(IconName::ArrowDown)
                            .disabled(pulling || self.busy)
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
                    .h(px(120.))
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
                                        .child("Pull logs will appear here."),
                                )
                            } else {
                                list.children(log_lines)
                            }
                        }),
                    ),
            )
    }

    fn render_images(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_3()
            .child(self.render_pull_panel(cx))
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} images", self.images.len())),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("run-image")
                                    .small()
                                    .primary()
                                    .label("Run")
                                    .disabled(
                                        self.busy
                                            || self.pull_phase == PullPhase::Pulling
                                            || self.selected_image().is_none(),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let Some(img) = this.selected_image().cloned() else {
                                            return;
                                        };
                                        let reference = img.reference();
                                        this.image_input.update(cx, |state, cx| {
                                            state.set_value(&reference, window, cx);
                                        });
                                        this.section = Section::Run;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("remove-image")
                                    .small()
                                    .danger()
                                    .label("Remove")
                                    .disabled(
                                        self.busy
                                            || self.pull_phase == PullPhase::Pulling
                                            || self.selected_image().is_none(),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let Some(img) = this.selected_image().cloned() else {
                                            return;
                                        };
                                        let reference = img.reference();
                                        this.with_busy(cx, move |client| {
                                            client.remove_image(&reference)?;
                                            Ok(format!("Removed {reference}"))
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
                                "No images",
                                "Type an image name above and pull it, then refresh.",
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
                            .label("Create")
                            .icon(IconName::Plus)
                            .disabled(self.busy || self.pull_phase == PullPhase::Pulling)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let name = this.volume_input.read(cx).value().to_string();
                                this.with_busy(cx, move |client| {
                                    client.create_volume(&name)?;
                                    Ok(format!("Created volume {name}"))
                                });
                            })),
                    )
                    .child(
                        Button::new("remove-volume")
                            .danger()
                            .label("Remove")
                            .disabled(
                                self.busy
                                    || self.pull_phase == PullPhase::Pulling
                                    || self.selected_volume().is_none(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                let Some(vol) = this.selected_volume().cloned() else {
                                    return;
                                };
                                let name = vol.name;
                                this.with_busy(cx, move |client| {
                                    client.remove_volume(&name)?;
                                    Ok(format!("Removed {name}"))
                                });
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} volumes", self.volumes.len())),
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
                                "No volumes",
                                "Create a named volume above, or refresh after creating one with wslc volume create.",
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

    fn render_run(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_4()
            .max_w(px(520.))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Start a detached container through the local wslc CLI. Without WSLc, demo mode only updates the in-memory list.",
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Image"))
                    .child(Input::new(&self.image_input)),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Name (optional)"))
                    .child(Input::new(&self.name_input)),
            )
            .child(
                h_flex().gap_2().child(
                    Button::new("run-detached")
                        .primary()
                        .label("Run detached")
                        .icon(IconName::Play)
                        .disabled(self.busy || self.pull_phase == PullPhase::Pulling)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let image = this.image_input.read(cx).value().to_string();
                            let name = this.name_input.read(cx).value().to_string();
                            let name_opt = if name.trim().is_empty() {
                                None
                            } else {
                                Some(name)
                            };
                            this.with_busy(cx, move |client| {
                                let launched =
                                    client.run_detached(&image, name_opt.as_deref())?;
                                Ok(format!("Launched {launched}"))
                            });
                            this.section = Section::Containers;
                        })),
                ),
            )
            .child(
                div()
                    .mt_4()
                    .p_3()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary.opacity(0.35))
                    .child(
                        v_flex()
                            .gap_2()
                            .child(div().font_medium().text_sm().child("CLI equivalents"))
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(cx.theme().muted_foreground)
                                    .child("wslc pull <image>"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(cx.theme().muted_foreground)
                                    .child("wslc run -d --name <name> <image>"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(cx.theme().muted_foreground)
                                    .child("wslc volume list"),
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
                    .child(format!("binary: {}", self.status.binary)),
            )
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let section_index = self.section as usize;

        h_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(self.render_sidebar(cx))
            .child(
                v_flex()
                    .size_full()
                    .min_w_0()
                    .child(
                        TitleBar::new().child(
                            TabBar::new("section-tabs")
                                .mt(px(1.))
                                .segmented()
                                .selected_index(section_index)
                                .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                    this.section = Section::from_index(*ix);
                                    cx.notify();
                                }))
                                .child(Tab::new().label("Containers"))
                                .child(Tab::new().label("Images"))
                                .child(Tab::new().label("Volumes"))
                                .child(Tab::new().label("Run")),
                        ),
                    )
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
                                        Section::Images => panel.child(self.render_images(cx)),
                                        Section::Volumes => panel.child(self.render_volumes(cx)),
                                        Section::Run => panel.child(self.render_run(cx)),
                                    }
                                }),
                            ),
                    )
                    .child(self.render_status_bar(cx)),
            )
    }
}
