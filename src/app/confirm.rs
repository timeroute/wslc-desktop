
use gpui_kit::component::{
    ActiveTheme, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
};
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

    pub(crate) fn submit_confirm(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        match action {
            ConfirmAction::RemoveContainer { key, .. } => {
                let template = self.t().removed;
                self.with_busy(cx, move |client| {
                    client.remove_container(&key)?;
                    Ok(i18n::fmt1(template, "name", &key))
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
        let body = match action {
            ConfirmAction::RemoveContainer { label, .. } => {
                i18n::fmt1(t.confirm_remove_container, "name", label)
            }
            ConfirmAction::RemoveImage { reference } => {
                i18n::fmt1(t.confirm_remove_image, "name", reference)
            }
            ConfirmAction::RemoveVolume { name } => {
                i18n::fmt1(t.confirm_remove_volume, "name", name)
            }
        };

        modal_scrim(
            "confirm-remove-modal",
            cx,
            modal_panel(cx)
                .w(px(420.))
                .child(div().font_semibold().text_base().child(title))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(body),
                )
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
