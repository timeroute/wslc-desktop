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

impl AppView {
    pub(crate) fn render_volume_row(
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

    pub(crate) fn render_volumes(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
}
