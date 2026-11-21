//! Initial views for the requesting screen.
use super::COUNT_NODE;
use crate::{open_count, page_tree, starter_tree, TodoApp, DIALOG_ROOT, PAGE_ROOT};
use foundation_ui_runtime::{
    html,
    view::{anchor, template_types::Html},
};

pub(super) fn web_page(state: TodoApp) -> (Html, TodoApp) {
    let page = anchor(PAGE_ROOT, page_tree(&state, true, false).0);
    (page, state)
}

pub(super) fn starter(count: usize) -> Html {
    anchor(PAGE_ROOT, starter_tree(count))
}

pub(super) fn mobile_page(state: TodoApp, channel_id: u64, cursor: u64) -> (Html, TodoApp) {
    let page = anchor(PAGE_ROOT, page_tree(&state, false, false).0);
    let dialog = anchor(DIALOG_ROOT, html! {<Stack/>});
    let count = anchor(
        COUNT_NODE,
        open_count(state.items.iter().filter(|item| !item.done).count()),
    );

    let screen = html! { <Stack>{page}{dialog}{count}
        <mount-stream protocol="frame" transport="sse" api="/native-live" data-channel-id={channel_id} data-resume-from={cursor}/>
    </Stack> };
    (screen, state)
}
