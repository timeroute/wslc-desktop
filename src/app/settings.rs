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
    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.t();
        let locale = self.locale;
        let theme_mode = self.theme_mode;

        v_flex()
            .size_full()
            .gap_6()
            .max_w(px(560.))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t.settings_intro),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary.opacity(0.28))
                    .child(div().font_medium().text_sm().child(t.language))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.language_hint),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("lang-zh")
                                    .label(Locale::Zh.label())
                                    .when(locale == Locale::Zh, |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_locale(Locale::Zh, window, cx);
                                    })),
                            )
                            .child(
                                Button::new("lang-en")
                                    .label(Locale::En.label())
                                    .when(locale == Locale::En, |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_locale(Locale::En, window, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary.opacity(0.28))
                    .child(div().font_medium().text_sm().child(t.theme))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.theme_hint),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("theme-dark")
                                    .label(t.theme_dark)
                                    .when(theme_mode == ThemeMode::Dark, |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_theme(ThemeMode::Dark, window, cx);
                                    })),
                            )
                            .child(
                                Button::new("theme-light")
                                    .label(t.theme_light)
                                    .when(theme_mode == ThemeMode::Light, |b| b.primary())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_theme(ThemeMode::Light, window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
