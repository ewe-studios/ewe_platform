//! Store each pending request under its operation identifier.
use super::{error, ActionError, Context, Entries, ErrorTrace, Pending, PendingChange};
#[cfg(not(all(feature = "web", target_family = "wasm")))]
use foundation_db::{KeyValueStore, MemoryStorage};
#[cfg(all(feature = "mobile", not(target_family = "wasm")))]
use foundation_ui_native::storage::NativeRecordFile;
#[cfg(all(feature = "web", target_family = "wasm"))]
use foundation_wasm_ui::storage::{HostStorage, KeyValueStore};
use std::collections::BTreeMap;

const PREFIX: &str = "pending/";
#[cfg(any(
    all(feature = "web", target_family = "wasm"),
    all(feature = "mobile", not(target_family = "wasm"))
))]
const RECEIPTS: &str = "imported/v1/";
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

fn insert_record(entries: &mut Entries, key: &str, value: Pending) -> Result<()> {
    let id = key
        .strip_prefix(PREFIX)
        .ok_or_else(|| error("Invalid pending Todo key"))?;
    validate(id, &value)?;
    entries.insert(id.to_owned(), value);
    Ok(())
}

async fn list(store: &impl KeyValueStore) -> Result<Entries> {
    use futures_util::TryStreamExt;
    let mut keys = store.list_keys(Some(PREFIX)).await.map_err(storage_error)?;
    let mut entries = Entries::new();
    while let Some(key) = keys.try_next().await.map_err(storage_error)? {
        if let Some(value) = store.get::<Pending>(&key).await.map_err(storage_error)? {
            insert_record(&mut entries, &key, value)?;
        }
    }
    Ok(entries)
}

#[cfg(all(feature = "web", target_family = "wasm"))]
pub(crate) async fn migrate(ctx: foundation_context::Ctx) -> std::result::Result<(), String> {
    let storage = ctx.get::<HostStorage>().ok_or("Missing browser storage")?;
    if let Some(old) = storage
        .legacy_record::<Entries>("ewe.todo.pending.v1")
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
    #[cfg(all(feature = "web", target_family = "wasm"))]
    let storage = ctx
        .get::<HostStorage>()
        .ok_or_else(|| error("Missing browser storage"))?;
    #[cfg(not(all(feature = "web", target_family = "wasm")))]
    let storage = {
        #[cfg(all(feature = "mobile", not(target_family = "wasm")))]
        if let Some(file) = ctx.get::<NativeRecordFile>() {
            let old = file
                .legacy_record::<Entries>()
                .await
                .map_err(storage_error)?;
            if let Some(entries) = &old {
                for (key, value) in entries {
                    validate(key, value)?;
                }
            }
            let db = file.database(ctx.inner()).await.map_err(storage_error)?;
            if let Some(entries) = old {
                db.import_once(PREFIX, RECEIPTS, &entries)
                    .await
                    .map_err(storage_error)?;
            }
            use futures_util::TryStreamExt;
            let mut records = db.records::<Pending>(PREFIX).await.map_err(storage_error)?;
            let mut entries = Entries::new();
            while let Some((key, value)) = records.try_next().await.map_err(storage_error)? {
                insert_record(&mut entries, &key, value)?;
            }
            return Ok(entries);
        }
        ctx.get::<MemoryStorage>()
            .ok_or_else(|| error("Missing Todo memory storage"))?
    };
    list(&*storage).await
}

pub(super) async fn save(ctx: &Context, previous: &Entries, current: &Entries) -> Result<()> {
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
    #[cfg(all(feature = "web", target_family = "wasm"))]
    {
        ctx.get::<HostStorage>()
            .ok_or_else(|| error("Missing browser storage"))?
            .change_records(PREFIX, &inserts, &updates, &removals)
            .await
            .map_err(storage_error)
    }
    #[cfg(not(all(feature = "web", target_family = "wasm")))]
    {
        #[cfg(all(feature = "mobile", not(target_family = "wasm")))]
        if let Some(file) = ctx.get::<NativeRecordFile>() {
            let db = file.database(ctx.inner()).await.map_err(storage_error)?;
            return db
                .change_records(PREFIX, &inserts, &updates, &removals)
                .await
                .map_err(storage_error);
        }
        ctx.get::<MemoryStorage>()
            .ok_or_else(|| error("Missing Todo memory storage"))?
            .change_records(PREFIX, &inserts, &updates, &removals)
            .map_err(storage_error)
    }
}
