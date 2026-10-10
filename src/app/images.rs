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
    pub(crate) fn open_pull_modal(&mut self, cx: &mut Context<Self>) {
        if self.sdk_locked() {
            return;
        }
        self.pull_modal_open = true;
        if self.pull_phase == PullPhase::Idle {
            self.pull_status = self.t().pull_hint.into();
        }
        cx.notify();
    }

    pub(crate) fn dismiss_pull_modal(&mut self, cx: &mut Context<Self>) {
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
    pub(crate) fn append_pull_log(&mut self, line: impl Into<SharedString>) {
        self.pull_logs.push(line.into());
        if self.pull_logs.len() > MAX_PULL_LOG_LINES {
            let drop = self.pull_logs.len() - MAX_PULL_LOG_LINES;
            self.pull_logs.drain(0..drop);
        }
    }

    pub(crate) fn begin_pull(&mut self, reference: String, cx: &mut Context<Self>) {
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
    pub(crate) fn poll_pull(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(rx) = self.pull_rx.take() else {
            return false;
        };

        let (events, disconnected) = recv_all(&rx);

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
    pub(crate) fn render_pull_badge(&self, cx: &Context<Self>) -> impl IntoElement {
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
    pub(crate) fn render_image_row(
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
    pub(crate) fn render_pull_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

        modal_scrim(
            "pull-modal",
            cx,
            modal_panel(cx)
                .w(px(560.))
                .max_h(px(520.))
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
    pub(crate) fn render_images(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
}
