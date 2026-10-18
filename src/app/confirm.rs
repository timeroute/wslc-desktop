
use gpui_kit::component::{
    ActiveTheme, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::i18n::{self};

use super::state::{ConfirmAction, PullPhase};
use super::widgets::{modal_panel, modal_scrim};
use super::AppView;

impl AppView {
    pub(crate) fn request_confirm(&mut self, action: ConfirmAction, cx: &mut Context<Self>) {
        if self.sdk_locked()
            || self.busy
            || self.pull_phase == PullPhase::Pulling
            || self.confirm.is_some()
            || self.close_prompt_open
        {
            return;
        }
        self.confirm = Some(action);
        cx.notify();
    }

    pub(crate) fn dismiss_confirm(&mut self, cx: &mut Context<Self>) {
        self.confirm = None;
        cx.notify();
    }

    pub(crate) fn toggle_confirm_remove_volumes(&mut self, cx: &mut Context<Self>) {
        if let Some(ConfirmAction::RemoveContainer {
            remove_volumes,
            named_volumes,
            ..
        }) = self.confirm.as_mut()
        {
            if !named_volumes.is_empty() {
                *remove_volumes = !*remove_volumes;
                cx.notify();
            }
        }
    }

    pub(crate) fn submit_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        match action {
            ConfirmAction::RemoveContainer {
                key,
                remove_volumes,
                named_volumes,
                ..
            } => {
                let removed = self.t().removed;
                let removed_with_volumes = self.t().removed_container_with_volumes;
                self.with_busy(cx, move |client| {
                    client.remove_container(&key)?;
                    if remove_volumes {
                        let mut failed = Vec::new();
                        for name in &named_volumes {
                            if let Err(err) = client.remove_volume(name) {
                                failed.push(format!("{name}: {err}"));
                            }
                        }
                        if failed.is_empty() {
                            Ok(i18n::fmt2(
                                removed_with_volumes,
                                "name",
                                &key,
                                "volumes",
                                named_volumes.join(", "),
                            ))
                        } else {
                            Err(anyhow::anyhow!(
                                "container removed; volume delete failed: {}",
                                failed.join("; ")
                            ))
                        }
                    } else {
                        Ok(i18n::fmt1(removed, "name", &key))
                    }
                });
            }
            ConfirmAction::RemoveImage { reference } => {
                let template = self.t().removed;
                self.with_busy(cx, move |client| {
                    client.remove_image(&reference)?;
                    Ok(i18n::fmt1(template, "name", &reference))
                });
            }
            ConfirmAction::RemoveVolume { name } => {
                let template = self.t().removed;
                self.with_busy(cx, move |client| {
                    client.remove_volume(&name)?;
                    Ok(i18n::fmt1(template, "name", &name))
                });
            }
        }
    }

    pub(crate) fn render_confirm_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let action = self
            .confirm
            .as_ref()
            .expect("confirm modal rendered only when open");
        let title = t.confirm_remove_title;
        let (body, volumes_row) = match action {
            ConfirmAction::RemoveContainer {
                label,
                named_volumes,
                remove_volumes,
                ..
            } => {
                let body = i18n::fmt1(t.confirm_remove_container, "name", label);
                let volumes_row = if named_volumes.is_empty() {
                    None
                } else {
                    Some((
                        i18n::fmt1(
                            t.confirm_remove_container_volumes,
                            "volumes",
                            named_volumes.join(", "),
                        ),
                        *remove_volumes,
                    ))
                };
                (body, volumes_row)
            }
            ConfirmAction::RemoveImage { reference } => {
                (i18n::fmt1(t.confirm_remove_image, "name", reference), None)
            }
            ConfirmAction::RemoveVolume { name } => {
                (i18n::fmt1(t.confirm_remove_volume, "name", name), None)
            }
        };

        modal_scrim(
            "confirm-remove-modal",
            cx,
            modal_panel(cx)
                .w(px(460.))
                .child(div().font_semibold().text_base().child(title))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(body),
                )
                .when_some(volumes_row, |panel, (hint, selected)| {
                    panel.child(
                        v_flex()
                            .gap_2()
                            .pt_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.confirm_remove_container_volumes_hint),
                            )
                            .child(
                                Button::new("confirm-toggle-volumes")
                                    .small()
                                    .label(hint)
                                    .when(selected, |b| b.primary())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_confirm_remove_volumes(cx);
                                    })),
                            ),
                    )
                })
                .child(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("confirm-cancel")
                                .small()
                                .label(t.cancel)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.dismiss_confirm(cx);
                                })),
                        )
                        .child(
                            Button::new("confirm-ok")
                                .small()
                                .danger()
                                .label(t.remove)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.submit_confirm(cx);
                                })),
                        ),
                ),
        )
    }
}
