//! Build the same initial view for every renderer.
use crate::{page_tree, starter_tree, TodoApp};
use foundation_ui_runtime::{html, view::template_types::Html};

pub(super) fn page(state: &TodoApp, stream: u64, cursor: u64) -> Html {
    let page = page_tree(state);
    html! { <Stack>{page}
        <mount-stream protocol="frame" transport="sse" api="/live" data-channel-id={stream} data-resume-from={cursor}/>
    </Stack> }
}

pub(super) fn starter(count: usize) -> Html {
    starter_tree(count)
}
