use gpui_kit::component::{ActiveTheme, StyledExt as _, v_flex};
use gpui_kit::*;

use super::AppView;

impl AppView {
    pub(crate) fn empty_state(
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
}

pub(crate) fn modal_scrim(
    id: impl Into<ElementId>,
    cx: &Context<AppView>,
    child: impl IntoElement,
) -> impl IntoElement {
    div()
        .id(id)
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(cx.theme().overlay)
        .occlude()
        .child(child)
}

pub(crate) fn modal_panel(cx: &Context<AppView>) -> Div {
    v_flex()
        .m_6()
        .p_5()
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .shadow_md()
}
