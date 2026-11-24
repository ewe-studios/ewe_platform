//! Add Android tool checksums after checking the vendor manifest and archive length.
use super::Pins;
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use quick_xml::{events::Event, Reader};
use sha2::Digest;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
};
use toml_edit::{ArrayOfTables, DocumentMut, Item};

#[derive(Args)]
pub(super) struct Options {
    /// Maximum seconds per download. The previous updater uses one minute.
    #[arg(long, default_value_t = 60)]
    timeout: u64,
}
const TOOL: &str = "http:android-sdk";
const REPOSITORY: &str = "https://dl.google.com/android/repository/";

#[derive(Default, PartialEq, Eq)]
struct Archive {
    filename: String,
    size: String,
    sha1: String,
}

fn collect(
    reader: impl BufRead,
    wanted: &BTreeSet<String>,
) -> Result<BTreeMap<String, Archive>, TaskError> {
    let mut xml = Reader::from_reader(reader);
    xml.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut archive = None;
    let mut checksum = false;
    let mut buffer = Vec::new();
    let mut found = BTreeMap::new();
    loop {
        match xml
            .read_event_into(&mut buffer)
            .map_err(TaskError::operation)?
        {
            Event::Start(e) => {
                let key = e.local_name().as_ref().to_owned();
                let parent = stack.last().map(String::as_str);
                if key == "complete"
                    && parent == Some("archive")
                    && stack.iter().rev().nth(1).map(String::as_str) == Some("archives")
                {
                    archive = Some(Archive::default());
                }
                if archive.is_some() && key == "checksum" && parent == Some("complete") {
                    checksum = e
                        .try_get_attribute("type")
                        .map_err(TaskError::operation)?
                        .is_some_and(|v| v.value.as_ref() == "sha1");
                }
                stack.push(key);
            }
            Event::Text(e) => {
                if let Some(record) = &mut archive {
                    if stack.iter().rev().nth(1).is_some_and(|s| s == "complete") {
                        let value = e.as_ref();
                        match stack.last().map(String::as_str) {
                            Some("url") => record.filename.push_str(value),
                            Some("size") => record.size.push_str(value),
                            Some("checksum") if checksum => record.sha1.push_str(value),
                            _ => {}
                        }
                    }
                }
            }
            Event::End(_) => {
                let key = stack.pop();
                if key.as_deref() == Some("complete") {
                    if let Some(item) = archive.take().filter(|i| wanted.contains(&i.filename)) {
                        if found.get(&item.filename).is_some_and(|old| old != &item) {
                            return Err(TaskError::operation(
                                "Conflicting archive records in the Android manifest",
                            ));
                        }
                        found.insert(item.filename.clone(), item);
                    }
                }
                if key.as_deref() == Some("checksum") {
                    checksum = false;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(found)
}
fn safe_filename(url: &str) -> Result<&str, TaskError> {
    url.strip_prefix(REPOSITORY)
        .filter(|name| {
            !name.is_empty()
                && !name.starts_with('.')
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        })
        .ok_or_else(|| TaskError::operation(format!("Unexpected Android archive URL {url}")))
}
fn selected<'a>(
    document: &'a mut DocumentMut,
    pin: &str,
) -> Result<&'a mut toml_edit::Table, TaskError> {
    let entries = document
        .get_mut("tools")
        .and_then(|v| v.get_mut(TOOL))
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| TaskError::operation("Missing Android lock entries"))?;
    let mut matches = ArrayOfTables::iter_mut(entries)
        .filter(|entry| entry.get("version").and_then(Item::as_str) == Some(pin));
    let version = matches
        .next()
        .ok_or_else(|| TaskError::operation(format!("Missing Android lock for version {pin}")))?;
    if matches.next().is_some() {
        return Err(TaskError::operation(format!(
            "Duplicate Android lock for version {pin}"
        )));
    }
    Ok(version)
}
fn curl(url: &str, timeout: u64) -> ProcessCommand {
    ProcessCommand::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
        ])
        .arg(timeout.to_string())
        .arg("--url")
        .arg(url)
}
async fn download(
    ctx: &TaskContext,
    url: &str,
    archive: &Archive,
    timeout: u64,
) -> Result<String, TaskError> {
    let size = archive.size.parse::<u64>().map_err(TaskError::operation)?;
    let command = curl(url, timeout)
        .arg("--max-filesize")
        .arg(size.to_string());
    let file = capture(ctx, command).await?;
    if file.metadata().map_err(TaskError::operation)?.len() != size {
        return Err(TaskError::operation(format!(
            "Wrong archive length for {url}"
        )));
    }
    let mut reader = BufReader::new(file);
    let mut one = sha1::Sha1::new();
    let mut two = sha2::Sha256::new();
    loop {
        let bytes = reader.fill_buf().map_err(TaskError::operation)?;
        if bytes.is_empty() {
            break;
        }
        let count = bytes.len();
        one.update(bytes);
        two.update(bytes);
        reader.consume(count);
    }
    if !archive
        .sha1
        .eq_ignore_ascii_case(&hex::encode(one.finalize()))
    {
        return Err(TaskError::operation(format!(
            "Archive differs from the Android manifest: {url}"
        )));
    }
    Ok(format!("sha256:{}", hex::encode(two.finalize())))
}

fn write(root: &Path, path: &Path, source: &str, content: &str) -> TaskResult {
    let _guard = crate::staging::lock(root).map_err(TaskError::operation)?;
    if fs::read_to_string(path).map_err(TaskError::operation)? != source {
        return Err(TaskError::operation(
            "The mise lockfile changed during download",
        ));
    }
    let mut stage = tempfile::NamedTempFile::new_in(root).map_err(TaskError::operation)?;
    stage
        .as_file()
        .set_permissions(path.metadata().map_err(TaskError::operation)?.permissions())
        .map_err(TaskError::operation)?;
    stage
        .write_all(content.as_bytes())
        .map_err(TaskError::operation)?;
    stage.as_file().sync_all().map_err(TaskError::operation)?;
    stage.persist(path).map_err(TaskError::operation)?;
    Ok(())
}

pub(super) async fn run(ctx: TaskContext, options: Options) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let pins: Pins = toml::from_str(
        &fs::read_to_string(host.root.join("mise.toml")).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)?;
    let path = host.root.join("mise.lock");
    let original = fs::read_to_string(&path).map_err(TaskError::operation)?;
    let mut doc: DocumentMut = original.parse().map_err(TaskError::operation)?;
    let table = selected(&mut doc, pins.tool(TOOL)?)?;
    let mut missing = Vec::new();
    for platform in ["linux-x64", "macos-arm64", "macos-x64", "windows-x64"] {
        let key = format!("platforms.{platform}");
        let entry = table
            .get(&key)
            .and_then(Item::as_table)
            .ok_or_else(|| TaskError::operation(format!("Missing Android platform {platform}")))?;
        if entry
            .get("checksum")
            .and_then(Item::as_str)
            .is_some_and(|s| !s.is_empty())
        {
            continue;
        }
        let url = entry
            .get("url")
            .and_then(Item::as_str)
            .ok_or_else(|| TaskError::operation(format!("Missing Android URL for {platform}")))?;
        missing.push((key, safe_filename(url)?.to_owned(), url.to_owned()));
    }
    if missing.is_empty() {
        return Ok(());
    }
    let wanted = missing.iter().map(|(_, name, _)| name.clone()).collect();
    let file = capture(
        &ctx,
        curl(&format!("{REPOSITORY}repository2-3.xml"), options.timeout),
    )
    .await?;
    let manifest = collect(BufReader::new(file), &wanted)?;
    for (key, name, url) in missing {
        let archive = manifest.get(&name).ok_or_else(|| {
            TaskError::operation(format!(
                "Archive {name} is missing from the Android manifest"
            ))
        })?;
        let entry = table[&key]
            .as_table_mut()
            .ok_or_else(|| TaskError::operation("Missing Android platform record"))?;
        entry.insert(
            "checksum",
            toml_edit::value(download(&ctx, &url, archive, options.timeout).await?),
        );
        entry.sort_values_by(|a, _, b, _| (a.get() != "checksum").cmp(&(b.get() != "checksum")));
    }
    write(&host.root, &path, &original, &doc.to_string())
}
