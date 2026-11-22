//! Build the same initial view for each receiving platform.
use crate::{page_tree, starter_tree, TodoApp, DIALOG_ROOT, PAGE_ROOT};
use foundation_ui_runtime::{
    html,
    view::{anchor, template_types::Html},
};

pub(super) fn page(state: TodoApp, stream: u64, cursor: u64) -> (Html, TodoApp) {
    let page = anchor(PAGE_ROOT, page_tree(&state, false).0);
    let dialog = anchor(DIALOG_ROOT, html! {<Stack id="dialog"/>});
    let screen = html! { <Stack>{page}{dialog}
        <mount-stream protocol="frame" transport="sse" api="/live" data-channel-id={stream} data-resume-from={cursor}/>
    </Stack> };
    (screen, state)
}

pub(super) fn starter(count: usize) -> Html {
    anchor(PAGE_ROOT, starter_tree(count))
}
