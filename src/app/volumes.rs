
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    label::Label,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::i18n::{self};
use crate::wslc::VolumeInfo;

use super::state::ConfirmAction;
use super::widgets::{
    list_header, modal_panel, modal_scrim, resource_list, resource_row, row_actions,
};
use super::AppView;

const VOLUME_SIZES_GIB: [u32; 4] = [1, 2, 5, 10];

impl AppView {
    pub(crate) fn open_volume_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sdk_locked() || self.ops_disabled() {
            return;
        }
        self.volume_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.volume_size_gib = 1;
        self.volume_modal_open = true;
        cx.notify();
    }

    pub(crate) fn dismiss_volume_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy && self.volume_modal_open {
            return;
        }
        self.volume_modal_open = false;
        self.volume_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        cx.notify();
    }

    pub(crate) fn submit_create_volume(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sdk_locked() || self.busy {
            return;
        }
        let name = self.volume_input.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.set_status(self.t().volume_name_empty, cx);
            return;
        }
        let size_bytes = u64::from(self.volume_size_gib) * 1024 * 1024 * 1024;
        let template = self.t().created_volume;
        self.volume_modal_open = false;
        self.volume_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.with_busy(cx, move |client| {
            client.create_volume(&name, size_bytes)?;
            Ok(i18n::fmt1(template, "name", &name))
        });
    }

    pub(crate) fn render_volume_row(
        &self,
        index: usize,
        item: &VolumeInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = self.t();
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
        let ops_off = self.ops_disabled();

        resource_row(("volume-row", index), selected, cx)
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
                    )
                    .child(
                        row_actions().child(
                            Button::new(("volume-remove", index))
                                .small()
                                .danger()
                                .label(t.remove)
                                .disabled(ops_off)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_volume = Some(index);
                                    let Some(vol) = this.volumes.get(index).cloned() else {
                                        return;
                                    };
                                    this.request_confirm(
                                        ConfirmAction::RemoveVolume { name: vol.name },
                                        cx,
                                    );
                                })),
                        ),
                    ),
            )
    }

    pub(crate) fn render_volume_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let creating = self.busy && self.volume_modal_open;
        let size_gib = self.volume_size_gib;

        modal_scrim(
            "create-volume-modal",
            cx,
            modal_panel(cx)
                .child(div().font_semibold().text_base().child(t.create_volume_title))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.create_volume_intro),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new(t.volume_name_label))
                        .child(Input::new(&self.volume_input)),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new(t.volume_size_label))
                        .child(
                            h_flex().gap_2().children(VOLUME_SIZES_GIB.into_iter().map(|gib| {
                                Button::new(("volume-modal-size", gib as usize))
                                    .small()
                                    .label(crate::i18n::fmt1(
                                        t.volume_size_gib,
                                        "gib",
                                        gib.to_string(),
                                    ))
                                    .when(size_gib == gib, |b| b.primary())
                                    .disabled(creating)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.volume_size_gib = gib;
                                        cx.notify();
                                    }))
                                    .into_any_element()
                            })),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("volume-modal-cancel")
                                .label(t.close)
                                .disabled(creating)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.dismiss_volume_modal(window, cx);
                                })),
                        )
                        .child(
                            Button::new("volume-modal-submit")
                                .primary()
                                .label(if creating { t.working } else { t.create })
                                .icon(IconName::Plus)
                                .disabled(self.sdk_locked() || creating)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.submit_create_volume(window, cx);
                                })),
                        ),
                ),
        )
    }

    pub(crate) fn render_volumes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        v_flex()
            .size_full()
            .gap_3()
            .child(list_header(
                self.muted_count(
                    i18n::fmt1(t.volumes_count, "count", self.volumes.len().to_string()),
                    cx,
                ),
                Button::new("open-volume-modal")
                    .small()
                    .primary()
                    .label(t.create)
                    .icon(IconName::Plus)
                    .disabled(self.ops_disabled())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_volume_modal(window, cx);
                    })),
            ))
            .child(
                resource_list("volume-list").child({
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
}
