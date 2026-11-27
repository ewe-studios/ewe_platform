//! Store the creation receipt, current value or tombstone, and last sequence in one typed record.
use super::{
    mutations, CommitServices, CreationReceipt, Ctx, Deserialize, Mutex, RecordChange,
    Serialize, StoreState, Todo, TodoId, TodoStore,
};
use foundation_db::{KeyValueStore, LibsqlStore};
use futures_util::StreamExt;
use std::{collections::BTreeMap, fs::File, path::Path};

type Error = Box<dyn std::error::Error + Send + Sync>;
const PREFIX: &str = "todo.record/";

pub(super) struct Database {
    kv: LibsqlStore,
    // One live publisher owns the database while it serves its replay channel.
    _ownership: File,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    creation: CreationReceipt,
    current: Option<Todo>,
    position: u64,
}

impl Record {
    fn validate(&self, key: &str) -> Result<(), Error> {
        let original = &self.creation.item;
        if key != format!("{PREFIX}{}", original.id)
            || self.creation.position == 0
            || self.position < self.creation.position
            || (self.position == self.creation.position && self.current.as_ref() != Some(original))
            || (original.text != self.creation.submitted_title
                && original.text != self.creation.submitted_title.trim())
            || self
                .current
                .as_ref()
                .is_some_and(|row| row.id != original.id || row.text != original.text)
        {
            return Err(format!("Invalid Todo record at {key}").into());
        }
        Ok(())
    }
}

impl Database {
    async fn open(ctx: &Ctx, path: &Path) -> Result<Self, Error> {
        // Resolve an existing database before selecting its single-writer lock.
        let resolved;
        let path = match path.canonicalize() {
            Ok(target) => {
                resolved = target;
                resolved.as_path()
            }
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => path,
            Err(cause) => return Err(cause.into()),
        };
        let name = path.file_name().ok_or("The Todo store needs a file name")?;
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut lock_name = name.to_os_string();
        lock_name.push(".lock");
        let ownership = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(parent.join(lock_name))?;
        ownership.try_lock().map_err(|cause| {
            format!(
                "Cannot acquire the Todo store at {}: {cause}",
                path.display()
            )
        })?;
        let initialize = match path.metadata() {
            Ok(metadata) => metadata.len() == 0,
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => true,
            Err(cause) => return Err(cause.into()),
        };
        let kv = LibsqlStore::new_kv(
            path.to_str()
                .ok_or("The Todo store path requires valid UTF-8")?,
            None,
            ctx.with_empty_map(),
        )
        .await?;
        if initialize {
            kv.init_kv().await?;
        }
        Ok(Self {
            kv,
            _ownership: ownership,
        })
    }

    pub(super) async fn save(
        &self,
        creation: CreationReceipt,
        change: &RecordChange,
        position: u64,
    ) -> Result<(), Error> {
        let current = match change {
            RecordChange::Upsert(item) => Some(item.clone()),
            RecordChange::Remove(_) => None,
        };
        let key = format!("{PREFIX}{}", creation.item.id);
        self.kv
            .set(
                &key,
                Record {
                    creation,
                    current,
                    position,
                },
            )
            .await?;
        Ok(())
    }

    async fn read(&self) -> Result<StoreState, Error> {
        let mut state = StoreState::default();
        let mut order = BTreeMap::new();
        let mut positions = BTreeMap::new();
        let mut entries = self.kv.records::<Record>(PREFIX).await?;
        while let Some(entry) = entries.next().await {
            let (key, record) = entry?;
            record.validate(&key)?;
            let id = record.creation.item.id;
            for position in [record.creation.position, record.position] {
                if positions
                    .insert(position, id)
                    .is_some_and(|owner| owner != id)
                {
                    return Err("Two Todo records claim the same sequence".into());
                }
            }
            if let Some(current) = record.current {
                order.insert(record.creation.position, current);
            } else {
                state.retired_ids.insert(id);
                state.removed.insert(id, record.position);
            }
            state.position = state.position.max(record.position);
            state.creations.insert(id, record.creation);
        }
        state.items = order.into_values().collect();
        Ok(state)
    }
}

impl TodoStore {
    /// Open the confirmed records and creation receipts for one server.
    ///
    /// # Errors
    /// Return an error for an unreadable database, a malformed record, or an active writer.
    pub async fn open(ctx: &Ctx, path: &Path) -> Result<Self, Error> {
        let database = Database::open(ctx, path).await?;
        let state = database.read().await?;
        Ok(Self(
            Mutex::new(state),
            CommitServices {
                database: Some(database),
                writer: Some(mutations::Writer::start()?),
                ..Default::default()
            },
        ))
    }

    /// Return the last committed position, including positions of deleted records.
    #[must_use]
    pub fn position(&self) -> u64 {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .position
    }

    /// Read a confirmed record by identifier from the configured store.
    ///
    /// # Errors
    /// Return the database error when the lookup fails.
    pub async fn get(&self, id: TodoId) -> Result<Option<Todo>, Error> {
        if let Some(database) = &self.1.database {
            let _writer = self.1.gate.lock().await;
            let key = format!("{PREFIX}{id}");
            let Some(row) = database.kv.get::<Record>(&key).await? else {
                return Ok(None);
            };
            row.validate(&key)?;
            return Ok(row.current);
        }
        Ok(self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .items
            .iter()
            .find(|row| row.id == id)
            .cloned())
    }

    /// Import confirmed records in input order into an empty database.
    ///
    /// # Errors
    /// Reject invalid inputs or a nonempty destination. A failed transaction preserves the original database.
    pub async fn import(ctx: &Ctx, path: &Path, items: Vec<Todo>) -> Result<(), Error> {
        let mut records = BTreeMap::new();
        for (index, item) in items.into_iter().enumerate() {
            let id = item.id;
            let position = u64::try_from(index)?
                .checked_add(1)
                .ok_or("Too many Todo records")?;
            let record = Record {
                creation: CreationReceipt {
                    submitted_title: item.text.clone(),
                    item: item.clone(),
                    position,
                },
                current: Some(item),
                position,
            };
            record.validate(&format!("{PREFIX}{id}"))?;
            if records.insert(id.to_string(), record).is_some() {
                return Err(format!("Duplicate Todo identifier {id}").into());
            }
        }
        let database = Database::open(ctx, path).await?;
        if database.read().await?.position != 0 {
            return Err("The import destination already contains Todo records".into());
        }
        let inserts = records
            .iter()
            .map(|(key, record)| (key.as_str(), record))
            .collect();
        database
            .kv
            .change_records(PREFIX, &inserts, &BTreeMap::new(), &[])
            .await?;
        Ok(())
    }
}
