
use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Selectable as _, StyledExt as _, TitleBar,
    button::{Button, ButtonVariants as _},
    h_flex,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use gpui_kit::prelude::{FluentBuilder as _, InteractiveElement as _};
use gpui_kit::*;

use crate::i18n::{self};
use crate::wslc::BackendKind;

use super::state::Section;
use super::AppView;

impl AppView {
    pub(crate) fn render_engine_badge(&self, cx: &Context<Self>) -> impl IntoElement {
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
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                        SidebarMenuItem::new(t.overview)
                            .icon(IconName::LayoutDashboard)
                            .active(self.section == Section::Overview)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Overview;
                                cx.notify();
                            })),
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

    pub(crate) fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
    pub(crate) fn render_status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
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
}
