//! Resolve Cargo dependency names and preserve the manifest's existing layout.
use super::{invalid, Capability};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use toml_edit::{value, Array, DocumentMut, InlineTable, Item, Table, Value};

fn text<'a>(item: &'a Item, key: &str) -> Option<&'a str> {
    item.get(key).and_then(Item::as_str)
}

fn workspace(dir: &Path, manifest: &DocumentMut) -> io::Result<Option<(PathBuf, DocumentMut)>> {
    let explicit = manifest.get("package").and_then(|p| text(p, "workspace"));
    let dirs: Vec<PathBuf> = match explicit {
        Some(path) => vec![dir.join(path)],
        None => dir.ancestors().map(Path::to_owned).collect(),
    };
    for folder in dirs {
        let path = folder.join("Cargo.toml");
        let source = match fs::read_to_string(&path) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::NotFound && explicit.is_none() => continue,
            Err(e) => return Err(e),
        };
        let doc = source.parse::<DocumentMut>().map_err(invalid)?;
        if doc.contains_key("workspace") {
            return Ok(Some((folder, doc)));
        }
    }
    if explicit.is_some() {
        return Err(invalid("Cannot resolve package.workspace"));
    }
    Ok(None)
}

fn validate(item: &Item, base: &Path, provider: &Capability) -> io::Result<()> {
    if let Some(req) = item.as_str().or_else(|| text(item, "version")) {
        let req = semver::VersionReq::parse(req).map_err(invalid)?;
        if !req.matches(&semver::Version::parse(provider.version).map_err(invalid)?) {
            return Err(invalid(format!(
                "Incompatible version for {}",
                provider.package
            )));
        }
    }
    if let Some(path) = text(item, "path") {
        let local = base.join(path).canonicalize()?;
        if let Some(expected) = &provider.path {
            if local != expected.canonicalize()? {
                return Err(invalid("Conflicting build dependency path"));
            }
        }
        let doc = fs::read_to_string(local.join("Cargo.toml"))?
            .parse::<DocumentMut>()
            .map_err(invalid)?;
        if doc.get("package").and_then(|p| text(p, "name")) != Some(provider.package) {
            return Err(invalid(
                "Local build dependency has a different package name",
            ));
        }
    }
    if item.get("git").is_some()
        || item.get("registry").is_some()
        || item.get("optional").and_then(Item::as_bool) == Some(true)
    {
        return Err(invalid(
            "Conflicting build dependency source or optional setting",
        ));
    }
    Ok(())
}

pub(super) fn edit(dir: &Path, doc: &mut DocumentMut, provider: &Capability) -> io::Result<String> {
    let ws = workspace(dir, doc)?;
    let shared = ws
        .as_ref()
        .and_then(|(_, ws)| ws["workspace"].get("dependencies"))
        .and_then(Item::as_table_like);
    let parent = doc
        .entry("build-dependencies")
        .or_insert(Item::Table(Table::new()));
    let deps = parent
        .as_table_like_mut()
        .ok_or_else(|| invalid("Expected build-dependencies table"))?;
    let mut matches = Vec::new();
    for (key, item) in deps.iter() {
        let inherited = item.get("workspace").and_then(Item::as_bool) == Some(true);
        let base = if inherited {
            shared
                .and_then(|w| w.get(key))
                .ok_or_else(|| invalid(format!("Unresolved workspace dependency {key}")))?
        } else {
            item
        };
        if text(base, "package").unwrap_or(key) == provider.package {
            validate(
                base,
                if inherited {
                    &ws.as_ref().unwrap().0
                } else {
                    dir
                },
                provider,
            )?;
            matches.push(key.to_string());
        }
    }
    let alias = match &matches[..] {
        [alias] => alias.clone(),
        [] => {
            let inherited: Vec<_> = shared
                .into_iter()
                .flat_map(|t| t.iter())
                .filter(|(k, item)| text(item, "package").unwrap_or(k) == provider.package)
                .collect();
            let mut table = InlineTable::new();
            let alias = match &inherited[..] {
                [(key, item)] => {
                    validate(item, &ws.as_ref().unwrap().0, provider)?;
                    table.insert("workspace", Value::from(true));
                    *key
                }
                [] => {
                    table.insert("version", Value::from(provider.version));
                    if let Some(path) = &provider.path {
                        let path = path.canonicalize()?;
                        let relative = pathdiff::diff_paths(&path, dir).unwrap_or(path);
                        table.insert(
                            "path",
                            Value::from(
                                relative
                                    .to_str()
                                    .ok_or_else(|| invalid("Non-Unicode dependency path"))?,
                            ),
                        );
                    }
                    provider.package
                }
                _ => return Err(invalid("Ambiguous workspace dependency aliases")),
            };
            if deps.contains_key(alias) {
                return Err(invalid("Build dependency name collision"));
            }
            let declaration = Item::Value(Value::InlineTable(table));
            if declaration.get("workspace").is_none() {
                validate(&declaration, dir, provider)?;
            }
            deps.insert(alias, declaration);
            alias.to_owned()
        }
        _ => return Err(invalid("Ambiguous build dependency aliases")),
    };
    merge_features(deps, shared, &alias, provider.features)?;
    Ok(alias)
}

fn merge_features(
    deps: &mut dyn toml_edit::TableLike,
    shared: Option<&dyn toml_edit::TableLike>,
    alias: &str,
    features: &[&str],
) -> io::Result<()> {
    let inherited_features = deps
        .get(alias)
        .filter(|i| i.get("workspace").and_then(Item::as_bool) == Some(true))
        .and_then(|_| shared.and_then(|t| t.get(alias)))
        .and_then(|item| item.get("features"))
        .and_then(Item::as_array);
    let needed: Vec<_> = features
        .iter()
        .copied()
        .filter(|feature| {
            !inherited_features
                .is_some_and(|features| features.iter().any(|f| f.as_str() == Some(feature)))
        })
        .collect();
    if !needed.is_empty() {
        let item = deps.get_mut(alias).expect("resolved dependency");
        if let Some(version) = item.as_str() {
            let mut t = InlineTable::new();
            t.insert("version", Value::from(version));
            *item = Item::Value(Value::InlineTable(t));
        }
        let t = item
            .as_table_like_mut()
            .ok_or_else(|| invalid("Malformed build dependency"))?;
        let array = t
            .entry("features")
            .or_insert(value(Array::new()))
            .as_array_mut()
            .ok_or_else(|| invalid("Expected dependency feature array"))?;
        for feature in needed {
            if !array.iter().any(|f| f.as_str() == Some(feature)) {
                array.push(feature);
            }
        }
    }
    Ok(())
}
