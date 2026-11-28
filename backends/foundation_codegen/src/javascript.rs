//! Bundle local ECMAScript modules into classic browser scripts.
mod application;
use anyhow::{bail, Context, Result};
pub use application::{application, ApplicationScript};
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Mutex,
};
use swc_bundler::{Bundler, Config, Hook, Load, ModuleData, ModuleRecord, ModuleType, Resolve};
use swc_common::{
    comments::{CommentKind, SingleThreadedComments},
    sync::Lrc,
    FileName, Globals, SourceMap, Span, GLOBALS,
};
use swc_ecma_ast::{EsVersion, KeyValueProp, Program};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_loader::resolve::Resolution;
use swc_ecma_parser::{lexer::Lexer, EsSyntax, Parser, StringInput, Syntax};
use swc_ecma_transforms_base::helpers::Helpers;

/// Two representations of one script and the source files it imports.
pub struct JavaScriptBundle {
    pub readable: Vec<u8>,
    pub minified: Vec<u8>,
    pub inputs: Vec<PathBuf>,
}

struct Files<'a> {
    source: Lrc<SourceMap>,
    inputs: &'a Mutex<BTreeSet<PathBuf>>,
    notices: &'a Mutex<BTreeSet<String>>,
}
struct LocalImports;

impl Resolve for LocalImports {
    fn resolve(&self, base: &FileName, specifier: &str) -> Result<Resolution> {
        let FileName::Real(path) = base else {
            bail!("Expected a local entry file")
        };
        if !specifier.starts_with("./") && !specifier.starts_with("../") {
            bail!(
                "Unsupported module import {specifier:?} in {}",
                path.display()
            )
        }
        let sibling = path.with_file_name(specifier);
        Ok(Resolution {
            filename: FileName::Real(
                sibling
                    .canonicalize()
                    .with_context(|| format!("Cannot resolve {}", sibling.display()))?,
            ),
            slug: None,
        })
    }
}

impl Load for Files<'_> {
    fn load(&self, name: &FileName) -> Result<ModuleData> {
        let FileName::Real(path) = name else {
            bail!("Expected a local JavaScript file")
        };
        let fm = self
            .source
            .load_file(path)
            .with_context(|| format!("Cannot open {}", path.display()))?;
        let comments = SingleThreadedComments::default();
        let lexer = Lexer::new(
            Syntax::Es(EsSyntax::default()),
            EsVersion::Es2022,
            StringInput::from(&*fm),
            Some(&comments),
        );
        let mut parser = Parser::new_from(lexer);
        let module = parser
            .parse_module()
            .map_err(|e| anyhow::anyhow!("{}: {:?}", path.display(), e.kind()))?;
        if let Some(error) = parser.take_errors().first() {
            bail!("{}: {:?}", path.display(), error.kind())
        }
        self.inputs
            .lock()
            .map_err(|_| anyhow::anyhow!("input list unavailable"))?
            .insert(path.clone());
        let mut notices = self
            .notices
            .lock()
            .map_err(|_| anyhow::anyhow!("notice list unavailable"))?;
        let (leading, trailing) = comments.take_all();
        for list in [leading, trailing] {
            for comment in list.borrow().values().flatten() {
                let text = comment.text.as_ref();
                let words = text.to_ascii_lowercase();
                if text.starts_with('!')
                    || [
                        "@license",
                        "@preserve",
                        "@cc_on",
                        "copyright",
                        "spdx-license-identifier:",
                        "licensed under",
                    ]
                    .into_iter()
                    .any(|marker| words.contains(marker))
                {
                    notices.insert(match comment.kind {
                        CommentKind::Block => format!("/*{text}*/"),
                        CommentKind::Line => format!("//{text}"),
                    });
                }
            }
        }
        Ok(ModuleData {
            fm,
            module,
            helpers: Helpers::new(false),
        })
    }
}

struct ClassicScript;
impl Hook for ClassicScript {
    fn get_import_meta_props(
        &self,
        _span: Span,
        module: &ModuleRecord,
    ) -> Result<Vec<KeyValueProp>> {
        bail!(
            "import.meta is unavailable in classic scripts: {}",
            module.file_name
        )
    }
}

fn emit(
    program: &Program,
    source: Lrc<SourceMap>,
    notices: &BTreeSet<String>,
    compact: bool,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for notice in notices {
        bytes.extend_from_slice(notice.as_bytes());
        bytes.push(b'\n');
    }
    let mut emitter = Emitter {
        cfg: swc_ecma_codegen::Config::default()
            .with_target(EsVersion::Es2022)
            .with_minify(compact),
        cm: source.clone(),
        comments: None,
        wr: JsWriter::new(source, "\n", &mut bytes, None),
    };
    emitter.emit_program(program)?;
    Ok(bytes)
}

fn minify(readable: &[u8], source: Lrc<SourceMap>, notices: &BTreeSet<String>) -> Result<Vec<u8>> {
    use swc_common::Mark;
    use swc_ecma_minifier::{
        optimize,
        option::{CompressOptions, ExtraOptions, MangleOptions, MinifyOptions},
    };
    use swc_ecma_transforms_base::{fixer::fixer, resolver};

    // Resolve names in the final classic scope before shortening local bindings.
    let script = source.new_source_file(
        FileName::Custom("bundle.js".into()).into(),
        std::str::from_utf8(readable)?.to_owned(),
    );
    let lexer = Lexer::new(
        Syntax::Es(EsSyntax::default()),
        EsVersion::Es2022,
        StringInput::from(&*script),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let script = parser
        .parse_script()
        .map_err(|error| anyhow::anyhow!("Invalid classic script: {:?}", error.kind()))?;
    let unresolved = Mark::new();
    let top = Mark::new();
    let program = Program::Script(script).apply(resolver(unresolved, top, false));
    let options = MinifyOptions {
        compress: Some(CompressOptions::default()),
        mangle: Some(MangleOptions::default()),
        ..Default::default()
    };
    let program = optimize(
        program,
        source.clone(),
        None,
        None,
        &options,
        &ExtraOptions {
            unresolved_mark: unresolved,
            top_level_mark: top,
            mangle_name_cache: None,
        },
    )
    .apply(fixer(None));
    emit(&program, source, notices, true)
}

/// Resolve all local imports and emit readable and compact classic scripts.
///
/// # Errors
/// Returns an error for a missing file, invalid module or unsupported import.
pub fn bundle(entry: &Path) -> Result<JavaScriptBundle> {
    let source = Lrc::new(SourceMap::default());
    let globals = Globals::new();
    let entry = entry
        .canonicalize()
        .with_context(|| format!("Cannot resolve entry {}", entry.display()))?;
    let inputs = Mutex::new(BTreeSet::new());
    let notices = Mutex::new(BTreeSet::new());
    let loader = Files {
        source: source.clone(),
        inputs: &inputs,
        notices: &notices,
    };
    GLOBALS.set(&globals, || {
        let mut bundler = Bundler::new(
            &globals,
            source.clone(),
            loader,
            LocalImports,
            Config {
                module: ModuleType::Iife,
                ..Default::default()
            },
            Box::new(ClassicScript),
        );
        let mut outputs =
            bundler.bundle(HashMap::from([("main".to_owned(), FileName::Real(entry))]))?;
        if outputs.len() != 1 {
            bail!("Expected one classic script, got {}", outputs.len())
        }
        let module = outputs.remove(0).module;
        drop(bundler);
        let notices = notices
            .lock()
            .map_err(|_| anyhow::anyhow!("notice list unavailable"))?;
        let readable = emit(&Program::Module(module), source.clone(), &notices, false)?;
        let minified = minify(&readable, source, &notices)?;
        Ok(JavaScriptBundle {
            readable,
            minified,
            inputs: inputs
                .lock()
                .map_err(|_| anyhow::anyhow!("input list unavailable"))?
                .iter()
                .cloned()
                .collect(),
        })
    })
}
