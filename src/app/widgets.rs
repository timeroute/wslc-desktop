use gpui_kit::component::{ActiveTheme, StyledExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::AppView;

/// Shared width for confirm / close / most modal panels.
pub(crate) const MODAL_WIDTH: f32 = 480.;

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

    pub(crate) fn muted_count(
        &self,
        text: impl Into<SharedString>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(text.into())
    }
}

/// Selectable list row shell (Containers / Images / Volumes).
pub(crate) fn resource_row(
    id: impl Into<ElementId>,
    selected: bool,
    cx: &Context<AppView>,
) -> Stateful<Div> {
    div()
        .id(id)
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
}

/// Scrollable resource list body.
pub(crate) fn resource_list(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .w_full()
        .min_h_0()
        .overflow_y_scroll()
}

/// Per-row action cluster (occludes row click).
pub(crate) fn row_actions() -> Div {
    h_flex().gap_1().flex_shrink_0().occlude()
}

/// Section header: count on the left, toolbar actions on the right.
pub(crate) fn list_header(
    count: impl IntoElement,
    actions: impl IntoElement,
) -> impl IntoElement {
    h_flex()
        .items_center()
        .justify_between()
        .child(count)
        .child(actions)
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
        .w(px(MODAL_WIDTH))
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .shadow_md()
}

/// Compact status / phase pill (engine LIVE, pull phase, setup step).
pub(crate) fn status_badge(
    label: impl Into<SharedString>,
    color: Hsla,
    cx: &Context<AppView>,
) -> impl IntoElement {
    div()
        .px_2()
        .py_0p5()
        .rounded(cx.theme().radius)
        .bg(color.opacity(0.18))
        .text_color(color)
        .text_xs()
        .font_semibold()
        .child(label.into())
}

/// Bordered scrollable log body used by pull / setup / container logs modals.
pub(crate) fn scroll_log_pane(
    id: impl Into<ElementId>,
    height: f32,
    cx: &Context<AppView>,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(height))
        .w_full()
        .overflow_y_scroll()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background.opacity(0.65))
        .p_2()
}

/// Settings / overview card surface.
pub(crate) fn settings_card(cx: &Context<AppView>) -> Div {
    v_flex()
        .gap_3()
        .p_4()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary.opacity(0.28))
}

/// Settings card with shared title + muted hint header.
pub(crate) fn settings_card_section(
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
    cx: &Context<AppView>,
) -> Div {
    settings_card(cx)
        .child(div().font_medium().text_sm().child(title.into()))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(hint.into()),
        )
}
