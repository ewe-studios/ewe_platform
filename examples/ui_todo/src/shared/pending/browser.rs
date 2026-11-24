//! Persist each pending browser operation under its original identifier.
use super::{error, ActionError, Context, Entries, ErrorTrace, Pending, PendingChange};
use foundation_wasm_ui::storage::{HostStorage, KeyValueStore};
use std::collections::BTreeMap;

const PREFIX: &str = "pending/";
const RECEIPTS: &str = "imported/v1/";
const LEGACY: &str = "ewe.todo.pending.v1";
type Result<T> = std::result::Result<T, ErrorTrace<ActionError>>;

fn storage_error(cause: impl std::fmt::Display) -> ErrorTrace<ActionError> {
    error(&cause.to_string())
}

fn validate(key: &str, pending: &Pending) -> Result<()> {
    let expected = match &pending.change {
        PendingChange::Add { id, .. } => format!("add:{id}"),
        PendingChange::Delete { id } => format!("delete:{id}"),
    };
    if key != expected {
        return Err(error("Pending Todo key differs from its change"));
    }
    Ok(())
}

async fn list(store: &impl KeyValueStore) -> Result<Entries> {
    use futures_util::TryStreamExt;
    let mut keys = store.list_keys(Some(PREFIX)).await.map_err(storage_error)?;
    let mut entries = Entries::new();
    while let Some(key) = keys.try_next().await.map_err(storage_error)? {
        if let Some(value) = store.get::<Pending>(&key).await.map_err(storage_error)? {
            let id = key
                .strip_prefix(PREFIX)
                .ok_or_else(|| error("Invalid pending Todo key"))?;
            validate(id, &value)?;
            entries.insert(id.to_owned(), value);
        }
    }
    Ok(entries)
}

pub(crate) async fn migrate(ctx: foundation_context::Ctx) -> std::result::Result<(), String> {
    let storage = ctx.get::<HostStorage>().ok_or("Missing browser storage")?;
    if let Some(old) = storage
        .legacy_record::<Entries>(LEGACY)
        .await
        .map_err(|cause| cause.to_string())?
    {
        for (key, value) in &old {
            validate(key, value).map_err(|cause| cause.current_context().message.clone())?;
        }
        storage
            .import_once(PREFIX, RECEIPTS, &old)
            .await
            .map_err(|cause| cause.to_string())?;
    }
    Ok(())
}

pub(super) async fn load(ctx: &Context) -> Result<Entries> {
    let storage = ctx
        .get::<HostStorage>()
        .ok_or_else(|| error("Missing browser storage"))?;
    list(&*storage).await
}

pub(super) async fn save(ctx: &Context, previous: &Entries, current: &Entries) -> Result<()> {
    let storage = ctx
        .get::<HostStorage>()
        .ok_or_else(|| error("Missing browser storage"))?;
    let mut inserts = BTreeMap::new();
    let mut updates = BTreeMap::new();
    for (key, value) in current {
        match previous.get(key) {
            Some(old) if old == value => {}
            Some(_) => {
                updates.insert(key.as_str(), value);
            }
            None => {
                inserts.insert(key.as_str(), value);
            }
        }
    }
    let removals: Vec<_> = previous
        .keys()
        .filter(|key| !current.contains_key(*key))
        .map(String::as_str)
        .collect();
    storage
        .change_records(PREFIX, &inserts, &updates, &removals)
        .await
        .map_err(storage_error)
}
