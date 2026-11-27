//! Declare the pending request records and actions for both roles.
use foundation_ui_runtime::action::{ActionResult, Context, Interaction};
use serde::{Deserialize, Serialize};

foundation_ui_runtime::__ewe_action_host! {
    @items
    mod host;
    pub(crate) mod records;
    pub(super) use host::{install, reconcile};
}

foundation_ui_runtime::__ewe_action_mobile! {
    @items
    pub(super) use host::restore;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingChange {
    Add {
        #[serde(alias = "creation_id")]
        id: super::TodoId,
        title: String,
    },
    Delete {
        id: super::TodoId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub change: PendingChange,
    pub retry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
}

#[foundation_ui_runtime::action(host)]
pub(super) async fn submit_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    host::submit_todo(ctx, event).await
}

#[foundation_ui_runtime::action(host)]
pub(super) async fn submit_delete(ctx: &Context, event: Interaction<()>) -> ActionResult {
    host::submit_delete(ctx, event).await
}

#[foundation_ui_runtime::action(host)]
pub(super) async fn retry_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    host::retry_todo(ctx, event).await
}
