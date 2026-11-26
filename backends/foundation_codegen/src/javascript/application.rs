//! Compile each application module once and preserve lazy evaluation of dynamic imports.
use super::{emit, minify, Files};
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use swc_bundler::Load;
use swc_common::{sync::Lrc, FileName, Globals, Mark, SourceMap, SyntaxContext, GLOBALS};
use swc_ecma_ast::{Callee, Expr, Ident, MemberExpr, MetaPropKind, Program};
use swc_ecma_transforms_base::{
    fixer::fixer,
    helpers::{Helpers, HELPERS},
    hygiene::hygiene,
    resolver,
};
use swc_ecma_transforms_module::{
    common_js::{common_js, FeatureFlag},
    util::{Config, ImportInterop},
};
use swc_ecma_visit::{VisitMut, VisitMutWith};

/// A classic application script and the files that Cargo must track.
pub struct ApplicationScript {
    pub bytes: Vec<u8>,
    pub inputs: Vec<PathBuf>,
}

struct Imports {
    unresolved: SyntaxContext,
    names: BTreeSet<String>,
    unsupported: bool,
}

impl VisitMut for Imports {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::MetaProp(meta) = expr {
            if meta.kind == MetaPropKind::ImportMeta {
                *expr = Expr::Member(MemberExpr {
                    span: meta.span,
                    obj: Box::new(Expr::Ident(Ident::new(
                        "require".into(),
                        meta.span,
                        self.unresolved,
                    ))),
                    prop: swc_ecma_ast::MemberProp::Ident(swc_ecma_ast::IdentName::new(
                        "meta".into(),
                        meta.span,
                    )),
                });
                return;
            }
        }
        if let Expr::Call(call) = expr {
            if matches!(&call.callee, Callee::Expr(f) if matches!(f.as_ref(), Expr::Ident(i) if i.sym == "require" && i.ctxt == self.unresolved))
            {
                match call.args.as_slice() {
                    [arg]
                        if arg.spread.is_none()
                            && arg.expr.as_lit().and_then(|v| v.as_str()).is_some() =>
                    {
                        if let Some(value) = arg
                            .expr
                            .as_lit()
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.value.as_str())
                        {
                            self.names.insert(value.to_owned());
                        } else {
                            self.unsupported = true;
                        }
                    }
                    _ => self.unsupported = true,
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

fn dependency(root: &Path, parent: &Path, name: &str) -> Result<PathBuf> {
    let path = if let Some(relative) = name.strip_prefix('/') {
        root.join(relative)
    } else if name.starts_with("./") || name.starts_with("../") {
        parent.join(name)
    } else {
        bail!("Unsupported application module {name:?}")
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("Cannot resolve {}", path.display()))?;
    ensure!(
        path.starts_with(root),
        "Module escapes the application root: {}",
        path.display()
    );
    Ok(path)
}

fn transform(module: swc_ecma_ast::Module) -> (Program, Imports) {
    let mark = Mark::new();
    let mut imports = Imports {
        unresolved: SyntaxContext::empty().apply_mark(mark),
        names: BTreeSet::new(),
        unsupported: false,
    };
    let transformed = HELPERS.set(&Helpers::new(false), || {
        let mut program = Program::Module(module)
            .apply(resolver(mark, Mark::new(), false))
            .apply(common_js(
                swc_ecma_transforms_module::path::Resolver::default(),
                mark,
                Config {
                    import_interop: Some(ImportInterop::None),
                    preserve_import_meta: true,
                    ..Default::default()
                },
                FeatureFlag {
                    support_block_scoping: true,
                    support_arrow: true,
                },
            ));
        program.visit_mut_with(&mut imports);
        program.apply(hygiene()).apply(fixer(None))
    });
    (transformed, imports)
}

const LOADER: &str = r#"
const cache = new Map();
const scriptUrl = globalThis.document?.currentScript?.src || globalThis.location?.href || "file:///";
function load(id) {
    if (cache.has(id)) {
        const prior = cache.get(id);
        if (prior.failed) throw prior.error;
        return prior.exports;
    }
    const module = {exports: Object.create(null)};
    cache.set(id, module);
    const [url, paths, evaluate] = modules[id];
    const require = name => load(paths[name]);
    require.meta = {url: new URL(url.split("/").map(encodeURIComponent).join("/"), scriptUrl).href};
    try { evaluate(require, module.exports); }
    catch (error) { module.failed = true; module.error = error; throw error; }
    return module.exports;
}
load(0);
})();"#;

/// Build one application script. Resolve root imports against the supplied public tree.
///
/// # Errors
/// Returns a source or import error if a module cannot enter the output script.
pub fn application(entry: &Path, public: &Path, compact: bool) -> Result<ApplicationScript> {
    let root = public.canonicalize()?;
    let entry = entry.canonicalize()?;
    ensure!(
        entry.starts_with(&root),
        "The application entry must belong to the public tree"
    );
    let source = Lrc::new(SourceMap::default());
    let inputs = Mutex::new(BTreeSet::new());
    let notices = Mutex::new(BTreeSet::new());
    let loader = Files {
        source: source.clone(),
        inputs: &inputs,
        notices: &notices,
    };
    let mut queue = vec![entry.clone()];
    let mut indices = BTreeMap::from([(entry, 0)]);
    let mut modules = String::from("(() => {\nconst modules = [\n");
    GLOBALS.set(&Globals::new(), || -> Result<()> {
        let mut index = 0;
        while index < queue.len() {
            let path = queue[index].clone();
            let data = loader.load(&FileName::Real(path.clone()))?;
            let (transformed, imports) = transform(data.module);
            ensure!(
                !imports.unsupported,
                "Computed module import in {}",
                path.display()
            );
            let mut dependencies = BTreeMap::new();
            for name in imports.names {
                let target = dependency(
                    &root,
                    path.parent().context("Missing module parent")?,
                    &name,
                )?;
                let id = *indices.entry(target.clone()).or_insert_with(|| {
                    queue.push(target);
                    queue.len() - 1
                });
                dependencies.insert(name, id);
            }
            let relative = path
                .strip_prefix(&root)?
                .to_str()
                .context("Non-Unicode module name")?
                .replace('\\', "/");
            let js = emit(&transformed, source.clone(), &BTreeSet::new(), false)?;
            writeln!(
                modules,
                "[{}, {}, function(require, exports) {{\n{}\n}}],",
                serde_json::to_string(&relative)?,
                serde_json::to_string(&dependencies)?,
                std::str::from_utf8(&js)?
            )?;
            index += 1;
        }
        Ok(())
    })?;
    modules.push_str("];\n");
    modules.push_str(LOADER);
    let notices = notices
        .into_inner()
        .map_err(|_| anyhow::anyhow!("Notice list unavailable"))?;
    let bytes = if compact {
        GLOBALS.set(&Globals::new(), || {
            minify(modules.as_bytes(), source, &notices)
        })?
    } else {
        let mut output = notices.into_iter().collect::<Vec<_>>().join("\n");
        output.push('\n');
        output.push_str(&modules);
        output.into_bytes()
    };
    Ok(ApplicationScript {
        bytes,
        inputs: queue,
    })
}
