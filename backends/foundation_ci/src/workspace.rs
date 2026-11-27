//! Build the workspace dependency graph from Cargo metadata.
use crate::{config::Policy, CiEnvironment, Settings};
use foundation_tasks::{OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput};
use futures_util::future::BoxFuture;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::File,
    io::{self, BufReader, Seek, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct Package {
    pub name: String,
    pub dir: String,
    pub needs_llama: bool,
    pub recipe: Option<crate::GroupRecipe>,
    pub build_members: Vec<String>,
    pub dependents: BTreeSet<String>,
    pub external: BTreeSet<String>,
    pub direct_deps: BTreeSet<String>,
    features: Option<BTreeSet<String>>,
}
pub struct Workspace {
    pub root: PathBuf,
    pub(crate) command_root: PathBuf,
    pub packages: BTreeMap<String, Package>,
    pub settings: BTreeMap<String, Settings>,
    pub(crate) policy: Policy,
}
#[derive(Deserialize)]
struct Metadata {
    workspace_root: PathBuf,
    workspace_members: Vec<String>,
    packages: Vec<MetadataPackage>,
    resolve: Resolve,
}
#[derive(Deserialize)]
struct MetadataPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<Dependency>,
    features: BTreeMap<String, Vec<String>>,
}
#[derive(Deserialize)]
struct Dependency {
    name: String,
    rename: Option<String>,
    optional: bool,
}
#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}
#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<Edge>,
}
#[derive(Deserialize)]
struct Edge {
    pkg: String,
}

struct Capture {
    file: Mutex<File>,
    forward: Arc<dyn TaskOutput>,
    combine: bool,
}
impl TaskOutput for Capture {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            if stream == OutputStream::Stderr && !self.combine {
                return self.forward.write(stream, bytes).await;
            }
            self.file
                .lock()
                .map_err(|_| io::Error::other("output lock poisoned"))?
                .write_all(bytes)
        })
    }
    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move { self.forward.flush().await })
    }
}
pub(crate) async fn capture(ctx: &TaskContext, cmd: ProcessCommand) -> Result<File, TaskError> {
    let mut file = tempfile::tempfile().map_err(TaskError::operation)?;
    capture_to(
        ctx,
        cmd,
        file.try_clone().map_err(TaskError::operation)?,
        false,
    )
    .await?;
    file.rewind().map_err(TaskError::operation)?;
    Ok(file)
}
/// Write child output to an open file. Send separate diagnostics to the task output.
pub(crate) async fn capture_to(
    ctx: &TaskContext,
    cmd: ProcessCommand,
    file: File,
    combine: bool,
) -> Result<(), TaskError> {
    let output = Capture {
        file: Mutex::new(file),
        forward: ctx.require::<TaskEnv>()?.output.clone(),
        combine,
    };
    ctx.exec_with_output(cmd, Arc::new(output)).await
}
impl Metadata {
    fn workspace_packages(self) -> Result<BTreeMap<String, Package>, TaskError> {
        let by_id: HashMap<_, _> = self.packages.iter().map(|p| (p.id.as_str(), p)).collect();
        let nodes: HashMap<_, _> = self
            .resolve
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), &n.deps))
            .collect();
        let members: HashSet<_> = self.workspace_members.iter().map(String::as_str).collect();
        let mut packages = BTreeMap::new();
        let mut reverse = Vec::new();
        for id in &self.workspace_members {
            let pkg = by_id
                .get(id.as_str())
                .ok_or_else(|| TaskError::operation(format!("Missing Cargo package {id}")))?;
            let mut seen = HashSet::new();
            let mut todo = vec![id.as_str()];
            let mut external = BTreeSet::new();
            let llama = |name: &str| {
                matches!(
                    name,
                    "infrastructure_llama_bindings" | "infrastructure_llama_cpp"
                )
            };
            let mut needs_llama = llama(&pkg.name);
            while let Some(next) = todo.pop() {
                for edge in *nodes.get(next).ok_or_else(|| {
                    TaskError::operation(format!("Missing dependency node {next}"))
                })? {
                    if !seen.insert(edge.pkg.as_str()) {
                        continue;
                    }
                    let dep = by_id.get(edge.pkg.as_str()).ok_or_else(|| {
                        TaskError::operation(format!("Unknown dependency {}", edge.pkg))
                    })?;
                    needs_llama |= llama(&dep.name);
                    if !members.contains(edge.pkg.as_str()) {
                        external.insert(dep.name.clone());
                    } else if dep.name != pkg.name {
                        reverse.push((dep.name.clone(), pkg.name.clone()));
                    }
                    todo.push(&edge.pkg);
                }
            }
            let dir = pkg
                .manifest_path
                .parent()
                .and_then(|p| p.strip_prefix(&self.workspace_root).ok())
                .ok_or_else(|| {
                    TaskError::operation(format!(
                        "Manifest outside workspace: {}",
                        pkg.manifest_path.display()
                    ))
                })?;
            let direct_deps = pkg
                .dependencies
                .iter()
                .flat_map(|d| std::iter::once(d.name.clone()).chain(d.rename.clone()))
                .collect();
            let features = pkg
                .features
                .keys()
                .cloned()
                .chain(
                    pkg.dependencies
                        .iter()
                        .filter(|d| d.optional)
                        .map(|d| d.rename.as_ref().unwrap_or(&d.name).clone()),
                )
                .collect();
            packages.insert(
                pkg.name.clone(),
                Package {
                    name: pkg.name.clone(),
                    recipe: None,
                    build_members: Vec::new(),
                    dir: if dir.as_os_str().is_empty() {
                        ".".into()
                    } else {
                        dir.to_string_lossy().replace('\\', "/")
                    },
                    needs_llama,
                    external,
                    direct_deps,
                    features: Some(features),
                    dependents: BTreeSet::new(),
                },
            );
        }
        for (name, dependent) in reverse {
            if let Some(p) = packages.get_mut(&name) {
                p.dependents.insert(dependent);
            }
        }
        Ok(packages)
    }
}
impl Workspace {
    fn add_groups(
        packages: &mut BTreeMap<String, Package>,
        root: &std::path::Path,
        policy: &Policy,
    ) -> Result<(), TaskError> {
        let members: BTreeSet<_> = packages.keys().cloned().collect();
        for (name, group) in &policy.groups {
            if members.contains(name) {
                return Err(TaskError::operation(format!(
                    "Build group {name} conflicts with a Cargo package"
                )));
            }
            if group.directory.as_os_str().is_empty()
                || !group
                    .directory
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            {
                return Err(TaskError::operation(format!(
                    "Build group {name} needs a relative directory inside the checkout"
                )));
            }
            let canonical = root
                .join(&group.directory)
                .canonicalize()
                .map_err(|e| TaskError::operation(format!("Group {name} directory: {e}")))?;
            let relative = canonical
                .strip_prefix(root)
                .ok()
                .filter(|dir| !dir.as_os_str().is_empty() && canonical.is_dir())
                .ok_or_else(|| {
                    TaskError::operation(format!(
                        "Group {name} directory is outside the checkout or is not a directory"
                    ))
                })?;
            let dir = relative.to_string_lossy().replace('\\', "/");
            if let Some(owner) = packages.values().find(|p| p.dir == dir) {
                return Err(TaskError::operation(format!(
                    "Group {name} directory already belongs to {}",
                    owner.name
                )));
            }
            let mut needs_llama = false;
            for dependency in &group.build_members {
                if !members.contains(dependency) {
                    return Err(TaskError::operation(format!(
                        "Build group {name} refers to unknown Cargo member {dependency}"
                    )));
                }
                needs_llama |= packages[dependency].needs_llama;
            }
            packages.insert(
                name.clone(),
                Package {
                    name: name.clone(),
                    dir,
                    needs_llama,
                    recipe: Some(group.recipe),
                    build_members: group.build_members.clone(),
                    dependents: BTreeSet::new(),
                    external: BTreeSet::new(),
                    direct_deps: BTreeSet::new(),
                    features: None,
                },
            );
        }
        for key in policy.packages.keys() {
            if !packages.contains_key(key) {
                return Err(TaskError::operation(format!(
                    "ci/packages.toml names unknown package {key}"
                )));
            }
        }
        Ok(())
    }

    /// Read package metadata and validate the build policy.
    ///
    /// # Errors
    /// Returns an error when Cargo, the group paths, or package settings are invalid.
    pub async fn load(ctx: &TaskContext) -> Result<Self, TaskError> {
        let env = ctx.require::<CiEnvironment>()?;
        let checkout = env
            .root
            .ancestors()
            .find(|p| p.join("ci/packages.toml").is_file())
            .ok_or_else(|| {
                TaskError::operation("Run this command in a checkout with ci/packages.toml")
            })?;
        let root = checkout.canonicalize().map_err(TaskError::operation)?;
        // Windows canonical paths add a verbatim prefix. Keep the caller's
        // spelling for tools that do not understand that prefix.
        let command_root = if cfg!(windows) { checkout } else { &root }.to_path_buf();
        let file = capture(
            ctx,
            ProcessCommand::new("cargo")
                .args(["metadata", "--format-version", "1"])
                .current_dir(&root),
        )
        .await?;
        let meta: Metadata =
            serde_json::from_reader(BufReader::new(file)).map_err(TaskError::operation)?;
        let mut packages = meta.workspace_packages()?;
        let policy: Policy = toml::from_str(
            &std::fs::read_to_string(root.join("ci/packages.toml"))
                .map_err(TaskError::operation)?,
        )
        .map_err(TaskError::operation)?;
        Self::add_groups(&mut packages, &root, &policy)?;
        let settings = packages
            .iter()
            .map(|(name, p)| Ok((name.clone(), policy.settings(name, p.features.as_ref())?)))
            .collect::<Result<_, TaskError>>()?;
        Ok(Self {
            root,
            command_root,
            packages,
            settings,
            policy,
        })
    }
}
