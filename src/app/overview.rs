
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    v_flex,
};
use gpui_kit::*;

use crate::wslc::BackendKind;

use super::state::Section;
use super::AppView;

const SESSION_NAME: &str = "wslc-desktop";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

impl AppView {
    pub(crate) fn render_overview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let running = self.containers.iter().filter(|c| c.is_running()).count();
        let (backend_label, backend_color) = match self.status.backend {
            BackendKind::Live => (t.overview_status_live, cx.theme().green),
            BackendKind::Unavailable => (t.overview_status_off, cx.theme().red),
        };
        let sdk_version = if self.status.version.is_empty() {
            t.overview_na.to_string()
        } else {
            self.status.version.clone()
        };
        let storage = std::env::var_os("LOCALAPPDATA")
            .map(|p| {
                std::path::PathBuf::from(p)
                    .join("WSLcDesktop")
                    .join("session")
                    .display()
                    .to_string()
            })
            .unwrap_or_else(|| r"%LOCALAPPDATA%\WSLcDesktop\session".into());

        v_flex()
            .size_full()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t.overview_intro),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_stretch()
                    .child(self.overview_card(
                        t.overview_engine,
                        cx,
                        v_flex()
                            .gap_3()
                            .child(self.overview_row(
                                t.overview_backend,
                                backend_label,
                                Some(backend_color),
                                cx,
                            ))
                            .child(self.overview_row(
                                t.overview_sdk_version,
                                &sdk_version,
                                None,
                                cx,
                            ))
                            .child(self.overview_row(
                                t.overview_app_version,
                                APP_VERSION,
                                None,
                                cx,
                            ))
                            .child(self.overview_row(
                                "wslcsdk",
                                &self.status.binary,
                                None,
                                cx,
                            ))
                            .child(self.overview_row(
                                t.overview_cpu,
                                if self.status.cpu_count == 0 {
                                    t.overview_na.to_string()
                                } else {
                                    self.status.cpu_count.to_string()
                                },
                                None,
                                cx,
                            ))
                            .child(self.overview_row(
                                t.overview_memory,
                                if self.status.memory_mb == 0 {
                                    t.overview_na.to_string()
                                } else {
                                    format!("{} MiB", self.status.memory_mb)
                                },
                                None,
                                cx,
                            )),
                    ))
                    .child(self.overview_card(
                        t.overview_resources,
                        cx,
                        v_flex()
                            .gap_3()
                            .child(self.overview_stat_row(
                                t.overview_containers,
                                self.containers.len().to_string(),
                                IconName::HardDrive,
                                cx,
                            ))
                            .child(self.overview_stat_row(
                                t.overview_running,
                                running.to_string(),
                                IconName::Play,
                                cx,
                            ))
                            .child(self.overview_stat_row(
                                t.overview_images,
                                self.images.len().to_string(),
                                IconName::Folder,
                                cx,
                            ))
                            .child(self.overview_stat_row(
                                t.overview_volumes,
                                self.volumes.len().to_string(),
                                IconName::Inbox,
                                cx,
                            )),
                    )),
            )
            .child(self.overview_card(
                t.overview_session,
                cx,
                v_flex()
                    .gap_3()
                    .child(self.overview_row(
                        t.overview_session_name,
                        SESSION_NAME,
                        None,
                        cx,
                    ))
                    .child(self.overview_row(
                        t.overview_storage,
                        &storage,
                        None,
                        cx,
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.status.message.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.overview_persist_hint),
                    ),
            ))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("overview-go-containers")
                            .small()
                            .primary()
                            .label(t.containers)
                            .icon(IconName::HardDrive)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Containers;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("overview-go-images")
                            .small()
                            .label(t.images)
                            .icon(IconName::Folder)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Images;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("overview-go-volumes")
                            .small()
                            .label(t.volumes)
                            .icon(IconName::Inbox)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.section = Section::Volumes;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn overview_card(
        &self,
        title: &'static str,
        cx: &Context<Self>,
        child: impl IntoElement,
    ) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_3()
            .p_4()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary.opacity(0.28))
            .child(div().font_medium().text_sm().child(title))
            .child(child)
    }

    fn overview_row(
        &self,
        label: &'static str,
        value: impl Into<SharedString>,
        value_color: Option<Hsla>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let color = value_color.unwrap_or(cx.theme().foreground);
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .child(
                div()
                    .text_sm()
                    .font_medium()
                    .text_color(color)
                    .truncate()
                    .child(value.into()),
            )
    }

    fn overview_stat_row(
        &self,
        label: &'static str,
        value: String,
        icon: IconName,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(icon).text_color(cx.theme().muted_foreground))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(label),
                    ),
            )
            .child(div().text_lg().font_semibold().child(value))
    }
}
