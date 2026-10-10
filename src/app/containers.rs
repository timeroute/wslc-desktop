
use gpui_kit::component::{
    ActiveTheme, Disableable as _, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    label::Label,
    v_flex,
};
use gpui_kit::prelude::{FluentBuilder as _, InteractiveElement as _};
use gpui_kit::*;

use crate::i18n::{self};
use crate::wslc::{
    ContainerInfo, CreateContainerRequest,
};

use super::state::{ConfirmAction, PullPhase};
use super::AppView;
use super::parsing::{parse_bind_mounts, parse_named_volumes, parse_port_mappings};
use super::widgets::{modal_panel, modal_scrim};

impl AppView {
    pub(crate) fn reset_create_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let t = self.t();
        self.image_input.update(cx, |state, cx| {
            state.set_value("nginx:alpine", window, cx);
        });
        self.name_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.set_placeholder(t.name_placeholder, window, cx);
        });
        self.ports_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.set_placeholder(t.ports_placeholder, window, cx);
        });
        self.bind_mounts_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.set_placeholder(t.bind_mounts_placeholder, window, cx);
        });
        self.named_volumes_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.set_placeholder(t.named_volumes_placeholder, window, cx);
        });
        self.command_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.set_placeholder(t.command_placeholder, window, cx);
        });
        self.create_auto_remove = false;
    }

    pub(crate) fn open_create_modal(
        &mut self,
        prefill_image: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sdk_locked() || self.ops_disabled() {
            return;
        }
        self.reset_create_form(window, cx);
        if let Some(image) = prefill_image {
            self.image_input.update(cx, |state, cx| {
                state.set_value(&image, window, cx);
            });
        }
        self.create_modal_open = true;
        cx.notify();
    }

    pub(crate) fn dismiss_create_modal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.create_modal_open = false;
        self.reset_create_form(window, cx);
        cx.notify();
    }

    pub(crate) fn dismiss_logs_modal(&mut self, cx: &mut Context<Self>) {
        self.logs_modal_open = false;
        self.logs_text = SharedString::default();
        self.logs_title = SharedString::default();
        cx.notify();
    }

    pub(crate) fn submit_create_container(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        let named_raw = self.named_volumes_input.read(cx).value().to_string();
        let command_raw = self.command_input.read(cx).value().to_string();
        let command_trim = command_raw.trim();
        let command_opt = if command_trim.is_empty() {
            None
        } else {
            Some(command_trim.to_string())
        };

        let ports = match parse_port_mappings(&ports_raw) {
            Ok(p) => p,
            Err(err) => {
                self.set_status(i18n::fmt1(t.parse_ports_failed, "err", err), cx);
                return;
            }
        };
        let mut volumes = match parse_bind_mounts(&mounts_raw) {
            Ok(v) => v,
            Err(err) => {
                self.set_status(i18n::fmt1(t.parse_bind_mounts_failed, "err", err), cx);
                return;
            }
        };
        match parse_named_volumes(&named_raw) {
            Ok(mut named) => volumes.append(&mut named),
            Err(err) => {
                self.set_status(i18n::fmt1(t.parse_named_volumes_failed, "err", err), cx);
                return;
            }
        }

        let request = CreateContainerRequest {
            image,
            name: name_opt,
            ports,
            volumes,
            auto_remove: self.create_auto_remove,
            command: command_opt,
        };

        let template = t.created_container;
        self.with_busy(cx, move |client| {
            let created = client.create_container(request)?;
            Ok(i18n::fmt1(template, "name", &created))
        });
    }

    pub(crate) fn open_container_logs(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.ops_disabled() {
            return;
        }
        let Some(item) = self.containers.get(index) else {
            return;
        };
        let key = Self::container_key(item);
        let title = key.clone();
        let failed = self.t().container_logs_failed;
        self.with_busy(cx, move |client| {
            let text = client
                .container_logs(&key)
                .map_err(|e| anyhow::anyhow!(i18n::fmt1(failed, "err", e.to_string())))?;
            Ok(format!("__LOGS__\0{title}\0{text}"))
        });
    }

    pub(crate) fn open_container_inspect(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.ops_disabled() {
            return;
        }
        let Some(item) = self.containers.get(index) else {
            return;
        };
        let key = Self::container_key(item);
        let title = format!("{} · inspect", key);
        let failed = self.t().container_inspect_failed;
        self.with_busy(cx, move |client| {
            let text = client
                .container_inspect(&key)
                .map_err(|e| anyhow::anyhow!(i18n::fmt1(failed, "err", e.to_string())))?;
            Ok(format!("__LOGS__\0{title}\0{text}"))
        });
    }

    pub(crate) fn open_exec_modal(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.ops_disabled() {
            return;
        }
        let Some(item) = self.containers.get(index) else {
            return;
        };
        let key = Self::container_key(item);
        let t = self.t();
        self.exec_target_key = key.into();
        self.exec_input.update(cx, |state, cx| {
            state.set_value("uname -a", window, cx);
            state.set_placeholder(t.container_exec_placeholder, window, cx);
        });
        self.exec_modal_open = true;
        cx.notify();
    }

    pub(crate) fn dismiss_exec_modal(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.exec_modal_open = false;
        self.exec_target_key = SharedString::default();
        cx.notify();
    }

    pub(crate) fn submit_exec(&mut self, cx: &mut Context<Self>) {
        if self.sdk_locked() || self.busy {
            return;
        }
        let key = self.exec_target_key.to_string();
        if key.is_empty() {
            return;
        }
        let command = self.exec_input.read(cx).value().to_string();
        let title = format!("{key} · exec");
        let failed = self.t().container_exec_failed;
        self.exec_modal_open = false;
        self.with_busy(cx, move |client| {
            let text = client
                .container_exec(&key, &command)
                .map_err(|e| anyhow::anyhow!(i18n::fmt1(failed, "err", e.to_string())))?;
            Ok(format!("__LOGS__\0{title}\0{text}"))
        });
    }
    pub(crate) fn render_container_row(
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
                                Button::new(("container-restart", index))
                                    .small()
                                    .label(t.restart)
                                    .disabled(ops_off)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.selected_container = Some(index);
                                        let key = this
                                            .containers
                                            .get(index)
                                            .map(Self::container_key)
                                            .unwrap_or_default();
                                        let template = this.t().restarted;
                                        this.with_busy(cx, move |client| {
                                            client.restart_container(&key)?;
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
                                Button::new(("container-inspect", index))
                                    .small()
                                    .label(t.inspect)
                                    .disabled(ops_off)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_container_inspect(index, cx);
                                    })),
                            )
                            .child(
                                Button::new(("container-exec", index))
                                    .small()
                                    .label(t.exec)
                                    .disabled(ops_off || !running)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_exec_modal(index, window, cx);
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
                                        let Some(item) = this.containers.get(index) else {
                                            return;
                                        };
                                        let key = Self::container_key(item);
                                        let label = if item.name.is_empty() {
                                            item.id.chars().take(12).collect::<String>()
                                        } else {
                                            item.name.clone()
                                        };
                                        this.request_confirm(
                                            ConfirmAction::RemoveContainer { key, label },
                                            cx,
                                        );
                                    })),
                            ),
                    ),
            )
    }

    pub(crate) fn render_containers(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                                        let key = Self::container_key(&c);
                                        let label = if c.name.is_empty() {
                                            c.id.chars().take(12).collect::<String>()
                                        } else {
                                            c.name.clone()
                                        };
                                        this.request_confirm(
                                            ConfirmAction::RemoveContainer { key, label },
                                            cx,
                                        );
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
    pub(crate) fn render_create_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let creating = self.busy && self.create_modal_open;
        let auto_remove = self.create_auto_remove;

        modal_scrim(
            "create-container-modal",
            cx,
            modal_panel(cx)
                .w(px(560.))
                .max_h(px(640.))
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
                        v_flex()
                            .gap_2()
                            .child(Label::new(t.named_volumes_label))
                            .child(Input::new(&self.named_volumes_input)),
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
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.dismiss_create_modal(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("create-submit")
                                    .primary()
                                    .label(if creating { t.working } else { t.create })
                                    .icon(IconName::Plus)
                                    .disabled(self.sdk_locked() || creating || self.pull_phase == PullPhase::Pulling)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.submit_create_container(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    pub(crate) fn render_exec_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let running = self.busy && self.exec_modal_open;
        modal_scrim(
            "exec-modal",
            cx,
            modal_panel(cx)
                .w(px(520.))
                .child(div().font_semibold().text_base().child(t.container_exec_title))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{} · {}", t.container_exec_intro, self.exec_target_key)),
                )
                .child(Input::new(&self.exec_input))
                .child(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("exec-cancel")
                                .small()
                                .label(t.cancel)
                                .disabled(running)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.dismiss_exec_modal(cx);
                                })),
                        )
                        .child(
                            Button::new("exec-run")
                                .small()
                                .primary()
                                .label(if running { t.working } else { t.container_exec_run })
                                .disabled(self.sdk_locked() || running)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.submit_exec(cx);
                                })),
                        ),
                ),
        )
    }

    pub(crate) fn render_logs_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let body: SharedString = if self.logs_text.is_empty() {
            t.logs_empty.into()
        } else {
            self.logs_text.clone()
        };
        let heading = if self.logs_title.to_ascii_lowercase().contains("inspect") {
            t.container_inspect_title
        } else if self.logs_title.to_ascii_lowercase().contains("exec") {
            t.container_exec_title
        } else {
            t.container_logs_title
        };

        modal_scrim(
            "logs-modal",
            cx,
            modal_panel(cx)
                .w(px(640.))
                .max_h(px(520.))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(div().font_semibold().text_base().child(heading))
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
}
