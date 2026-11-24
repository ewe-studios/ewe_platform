//! Select changed packages and their transitive dependents.
use crate::{workspace::capture, CiEnvironment, Workspace};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};
use toml::{Table, Value};
type Reasons = BTreeMap<String, String>;

fn changed_tables(
    old: Option<&str>,
    new: Option<&str>,
    workspace: bool,
) -> Option<BTreeSet<String>> {
    fn split(text: &str, workspace: bool) -> Option<(Table, Table)> {
        let mut doc: Table = toml::from_str(text).ok()?;
        let table = if workspace {
            doc.get_mut("workspace")
                .and_then(Value::as_table_mut)
                .and_then(|t| t.remove("dependencies"))
        } else {
            doc.remove("packages")
        };
        let entries = table.map_or(Some(Table::new()), |v| v.as_table().cloned())?;
        Some((doc, entries))
    }
    let (a, left) = split(old?, workspace)?;
    let (b, right) = split(new?, workspace)?;
    if a != b {
        return None;
    }
    Some(
        left.keys()
            .chain(right.keys())
            .filter(|k| left.get(*k) != right.get(*k))
            .cloned()
            .collect(),
    )
}
fn changed_lock(old: Option<&str>, new: Option<&str>) -> Option<BTreeSet<String>> {
    #[derive(serde::Deserialize, Ord, PartialOrd, Eq, PartialEq)]
    struct Record {
        name: String,
        version: Option<String>,
        source: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Lock {
        #[serde(default)]
        package: BTreeSet<Record>,
    }
    let a: Lock = toml::from_str(old?).ok()?;
    let b: Lock = toml::from_str(new?).ok()?;
    Some(
        a.package
            .symmetric_difference(&b.package)
            .map(|p| p.name.clone())
            .collect(),
    )
}
fn global(path: &str) -> bool {
    matches!(
        path,
        "rust-toolchain.toml"
            | "build.rs"
            | ".github/workflows/check.yaml"
            | "mise.toml"
            | "mise.lock"
            | ".gitmodules"
            | ".github/workflows/platform-images.yaml"
            | "backends/foundation_ci/Cargo.toml"
            | "backends/foundation_ci/src/lib.rs"
            | "backends/foundation_ci/src/main.rs"
            | "backends/foundation_ci/src/affected.rs"
            | "backends/foundation_ci/src/config.rs"
            | "backends/foundation_ci/src/execution.rs"
            | "backends/foundation_ci/src/images.rs"
            | "backends/foundation_ci/src/plan.rs"
            | "backends/foundation_ci/src/staging.rs"
            | "backends/foundation_ci/src/workspace.rs"
    ) || [
        ".cargo/",
        "ci/",
        "scripts/ci/",
        "tools/",
        "backends/foundation_ci/src/execution/",
        "backends/foundation_ci/src/images/",
    ]
    .iter()
    .any(|prefix| path.starts_with(prefix))
}
impl Workspace {
    async fn git(&self, ctx: &TaskContext, args: &[&str]) -> Result<String, TaskError> {
        let mut f = capture(
            ctx,
            ProcessCommand::new("git")
                .args(args.iter().copied())
                .current_dir(&self.root),
        )
        .await?;
        let mut data = String::new();
        f.read_to_string(&mut data).map_err(TaskError::operation)?;
        Ok(data)
    }
    async fn git_if(&self, ctx: &TaskContext, args: &[&str]) -> Result<Option<String>, TaskError> {
        match self.git(ctx, args).await {
            Ok(s) => Ok(Some(s)),
            Err(TaskError::Exit { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn base(&self, ctx: &TaskContext, base: Option<&str>) -> Result<String, TaskError> {
        let env = ctx.require::<CiEnvironment>()?;
        if let Some(b) = base.map(str::to_string).or_else(|| env.text("CI_BASE_SHA")) {
            return Ok(b);
        }
        if let Some(reference) = env.text("GITHUB_BASE_REF") {
            let remote = format!("origin/{reference}");
            if self
                .git_if(ctx, &["rev-parse", "--verify", "--quiet", &remote])
                .await?
                .is_none()
            {
                self.git_if(ctx, &["fetch", "--no-tags", "origin", &reference])
                    .await?;
            }
            return Ok(remote);
        }
        for reference in ["origin/master", "master"] {
            if self
                .git_if(ctx, &["rev-parse", "--verify", "--quiet", reference])
                .await?
                .is_some()
            {
                return Ok(self
                    .git_if(ctx, &["merge-base", "HEAD", reference])
                    .await?
                    .map_or_else(|| reference.to_owned(), |v| v.trim().to_owned()));
            }
        }
        Err(TaskError::operation(
            "cannot find a base: pass --base, or fetch origin/master",
        ))
    }
    fn everything(&self, why: &str) -> Reasons {
        self.packages
            .keys()
            .map(|n| (n.clone(), why.to_owned()))
            .collect()
    }
    fn owner(&self, path: &str) -> Option<&str> {
        self.packages
            .values()
            .filter(|p| {
                if p.dir == "." {
                    matches!(path.split('/').next(), Some("src" | "tests" | "benches"))
                } else {
                    path.strip_prefix(&p.dir)
                        .is_some_and(|rest| rest.starts_with('/'))
                }
            })
            .max_by_key(|p| p.dir.len())
            .map(|p| p.name.as_str())
    }
    async fn changed_files(
        &self,
        ctx: &TaskContext,
        path: &str,
        ancestor: &str,
        head: &str,
        include_local: bool,
    ) -> Result<(Option<String>, Option<String>), TaskError> {
        let old = self
            .git_if(ctx, &["show", &format!("{ancestor}:{path}")])
            .await?;
        let new = if include_local {
            match std::fs::read_to_string(self.root.join(path)) {
                Ok(text) => Some(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(TaskError::operation(e)),
            }
        } else {
            self.git_if(ctx, &["show", &format!("{head}:{path}")])
                .await?
        };
        Ok((old, new))
    }

    async fn changed_paths(
        &self,
        ctx: &TaskContext,
        base: &str,
        head: &str,
        include_local: bool,
    ) -> Result<(String, BTreeSet<String>), TaskError> {
        let ancestor = self
            .git_if(ctx, &["merge-base", base, head])
            .await?
            .map_or_else(|| base.to_owned(), |s| s.trim().to_owned());
        let mut changes = BTreeSet::new();
        let diff = self
            .git(ctx, &["diff", "--name-only", "-z", &ancestor, head])
            .await?;
        changes.extend(diff.split_terminator('\0').map(str::to_owned));
        if include_local {
            for args in [
                &["diff", "--name-only", "-z", "HEAD"][..],
                &["ls-files", "--others", "--exclude-standard", "-z"],
            ] {
                changes.extend(
                    self.git(ctx, args)
                        .await?
                        .split_terminator('\0')
                        .map(str::to_owned),
                );
            }
        }
        Ok((ancestor, changes))
    }

    fn expand_dependents(
        &self,
        deps: BTreeSet<String>,
        mut direct: Reasons,
        reasons: &mut Reasons,
    ) {
        for dep in deps {
            for (name, p) in &self.packages {
                if p.direct_deps.contains(&dep) {
                    direct
                        .entry(name.clone())
                        .or_insert_with(|| format!("the workspace dependency {dep} changed"));
                }
            }
        }
        for (name, why) in &direct {
            reasons.entry(name.clone()).or_insert_with(|| why.clone());
        }
        for (name, why) in direct {
            for dependent in &self.packages[&name].dependents {
                reasons
                    .entry(dependent.clone())
                    .or_insert_with(|| format!("depends on {name} ({why})"));
            }
        }
    }

    fn select_manifest_contracts(&self, path: &str, reasons: &mut Reasons) {
        if [
            "backends/foundation_installer/tests/fixtures/manifest/",
            "backends/foundation_registrar/tests/fixtures/manifest/",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
        {
            for pkg in self
                .packages
                .values()
                .filter(|pkg| pkg.recipe == Some(crate::GroupRecipe::ManifestContract))
            {
                reasons
                    .entry(pkg.name.clone())
                    .or_insert_with(|| format!("{path} changed"));
            }
        }
    }

    /// Select packages and record the first reason for each selection.
    ///
    /// # Errors
    /// Returns a repository or input error when the comparison cannot run.
    pub async fn affected(
        &self,
        ctx: &TaskContext,
        base: Option<&str>,
        head: Option<&str>,
    ) -> Result<(String, Reasons), TaskError> {
        let base = self.base(ctx, base).await?;
        let head = head.unwrap_or("HEAD");
        let include_local = head == "HEAD"
            && ctx
                .require::<CiEnvironment>()?
                .text("GITHUB_ACTIONS")
                .is_none();
        let (ancestor, changes) = self.changed_paths(ctx, &base, head, include_local).await?;
        let mut deps = BTreeSet::new();
        let mut direct = Reasons::new();
        let mut reasons = Reasons::new();
        for path in changes {
            if std::path::Path::new(&path)
                .extension()
                .is_some_and(|ext| ext == "md")
            {
                continue;
            }
            if matches!(
                path.as_str(),
                "Cargo.toml" | "ci/packages.toml" | "Cargo.lock"
            ) {
                let (old, new) = self
                    .changed_files(ctx, &path, &ancestor, head, include_local)
                    .await?;
                let pair = (old.as_deref(), new.as_deref());
                match path.as_str() {
                    "Cargo.lock" => {
                        let Some(mut names) = changed_lock(pair.0, pair.1) else {
                            return Ok((
                                base,
                                self.everything("Cargo.lock changed and could not be compared"),
                            ));
                        };
                        names.retain(|key| !self.packages.contains_key(key));
                        for (name, p) in &self.packages {
                            if let Some(dep) = names.intersection(&p.external).next() {
                                direct
                                    .entry(name.clone())
                                    .or_insert_with(|| format!("Cargo.lock changed {dep}"));
                            }
                        }
                    }
                    "Cargo.toml" => {
                        let Some(names) = changed_tables(pair.0, pair.1, true) else {
                            return Ok((base,self.everything("the root Cargo.toml workspace, patch or profile settings changed")));
                        };
                        deps.extend(names);
                    }
                    _ => {
                        let Some(names) = changed_tables(pair.0, pair.1, false) else {
                            return Ok((
                                base,
                                self.everything("ci/packages.toml [defaults] changed"),
                            ));
                        };
                        for name in names {
                            if self.packages.contains_key(&name) {
                                reasons
                                    .insert(name, "its section of ci/packages.toml changed".into());
                            }
                        }
                    }
                }
                continue;
            }
            if self.packages.contains_key("foundation_ui_wire")
                && ((path == "backends/foundation_ci/src/wire.rs"
                    || path.starts_with("backends/foundation_ci/src/wire/"))
                    || matches!(
                        path.as_str(),
                        "backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js"
                            | "pnpm-lock.yaml"
                            | "pnpm-workspace.yaml"
                    ))
            {
                direct
                    .entry("foundation_ui_wire".into())
                    .or_insert_with(|| format!("{path} affects the wire bindings"));
            } else if global(&path) {
                return Ok((base, self.everything(&format!("{path} changed"))));
            } else {
                self.select_manifest_contracts(&path, &mut reasons);
            }
            if let Some(owner) = self.owner(&path) {
                direct
                    .entry(owner.into())
                    .or_insert_with(|| format!("{path} changed"));
            }
        }
        self.expand_dependents(deps, direct, &mut reasons);
        Ok((base, reasons))
    }
}
