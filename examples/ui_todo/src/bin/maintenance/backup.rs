//! Read current backups and convert numeric records without changing the input.
use foundation_context::Ctx;
use serde::{de::DeserializeOwned, Deserialize};
use std::{
    collections::HashSet,
    fs::File,
    io::{self, BufReader, BufWriter, Read, Write},
    path::Path,
};
use ui_todo::{Todo, TodoId};

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    id: TodoId,
    text: String,
    done: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Legacy {
    id: u64,
    creation_id: Option<TodoId>,
    text: String,
    done: bool,
}

fn read<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<T, Error> {
    if !path.metadata()?.is_file() {
        return Err("The Todo backup must be a regular file".into());
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err("The Todo backup must be a regular file".into());
    }
    let too_large = || format!("The Todo backup exceeds the {limit}-byte limit");
    if metadata.len() > limit {
        return Err(too_large().into());
    }
    let mut reader = BufReader::new(file).take(limit.saturating_add(1));
    let records = serde_json::from_reader(&mut reader);
    if reader.limit() == 0 {
        return Err(too_large().into());
    }
    Ok(records?)
}

pub(super) fn restore(path: &Path, limit: u64) -> Result<Vec<Todo>, Error> {
    let records: Vec<Current> = read(path, limit).map_err(|error| {
        format!(
            "Cannot restore {}: {error}. Use todo-maintenance convert for numeric records.",
            path.display()
        )
    })?;
    Ok(records
        .into_iter()
        .map(|row| Todo {
            id: row.id,
            text: row.text,
            done: row.done,
        })
        .collect())
}

struct LimitedWriter<W> {
    output: W,
    remaining: u64,
}
impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(io::Error::other(
                "The converted Todo backup exceeds the restore size limit",
            ));
        }
        let written = self.output.write(bytes)?;
        self.remaining -= written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

pub(super) fn convert(
    ctx: &Ctx,
    input: &Path,
    output: &Path,
    limit: u64,
    mut mapping: impl Write,
) -> Result<(), Error> {
    let legacy: Vec<Legacy> = read(input, limit)
        .map_err(|e| format!("Cannot convert numeric backup {}: {e}", input.display()))?;
    let mut identifiers = HashSet::new();
    let mut known = HashSet::new();
    for item in &legacy {
        if item.id == 0 || !identifiers.insert(item.id) {
            return Err(format!("Invalid or duplicate numeric Todo identifier {}", item.id).into());
        }
        if let Some(id) = item.creation_id {
            if !known.insert(id) {
                return Err(format!("Duplicate creation identifier {id}").into());
            }
        }
    }
    let mut records = Vec::with_capacity(legacy.len());
    let mut converted = Vec::with_capacity(legacy.len());
    for item in legacy {
        let id = if let Some(id) = item.creation_id {
            id
        } else {
            let env = ctx
                .get::<foundation_context::env::Env>()
                .ok_or("Missing Todo entropy source")?;
            let id = env.entropy().uuid_v4()?;
            if !known.insert(id) {
                return Err(format!("Generated duplicate identifier {id}").into());
            }
            id
        };
        converted.push((item.id, id));
        records.push(Todo {
            id,
            text: item.text,
            done: item.done,
        });
    }
    let directory = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(directory)?;
    {
        let mut writer = BufWriter::new(LimitedWriter {
            output: temp.as_file_mut(),
            remaining: limit,
        });
        serde_json::to_writer(&mut writer, &records)?;
        writer.flush()?;
    }
    temp.as_file().sync_all()?;
    temp.persist_noclobber(output).map_err(|error| {
        format!(
            "Cannot create converted backup {}: {}",
            output.display(),
            error.error
        )
    })?;
    writeln!(mapping, "numeric_id\ttodo_id")?;
    for (old, id) in converted {
        writeln!(mapping, "{old}\t{id}")?;
    }
    mapping.flush()?;
    Ok(())
}
