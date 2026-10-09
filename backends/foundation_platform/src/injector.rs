//! `ScriptInjector` — platform runtime script injection (F24).
//!
//! WHY: `foundation_wasm`, `foundation_wasm_ui`, and polyfill runtimes need to
//! be injected into every webview before page scripts run. The platform owns
//! this — apps only ship their own code. The injector supports OTA-updatable
//! runtimes via a disk→embedded fallback chain.
//!
//! WHAT: `ScriptInjector` collects scripts with resolution chains.
//! At webview creation, each script is resolved (disk first, embedded
//! fallback) and evaluated via `window.eval()`.
//!
//! HOW: Scripts are registered on `ScriptInjector` during builder setup.
//! `inject_platform_runtimes()` registers the four standard runtime scripts
//! with their disk→static fallback chains. `inject_script()` adds custom
//! scripts. Resolution happens in `resolve_all()` at build time.

use std::path::PathBuf;

/// Where a script's source comes from.
#[derive(Debug, Clone)]
pub enum ScriptSource {
    /// Read from disk at runtime. OTA-updatable — replace the file and new
    /// webviews pick it up without an APK rebuild.
    Disk { relative_path: PathBuf },
    /// Static source compiled into the binary. Always available as fallback.
    Static { source: &'static str },
}

/// A script registered for injection into every webview.
#[derive(Debug, Clone)]
pub struct InjectedScript {
    /// Unique identifier (e.g. `"foundation_wasm"`, `"scheme_interceptor"`).
    pub id: String,
    /// Resolution chain — first source that resolves wins.
    pub sources: Vec<ScriptSource>,
}

/// Collection of scripts injected into every webview on creation.
///
/// Scripts are resolved in registration order. Each script's sources
/// are tried in order — first successful resolution wins.
#[derive(Debug, Default)]
pub struct ScriptInjector {
    /// Base directory for disk-based script resolution.
    pub resource_root: PathBuf,
    /// Registered scripts in injection order.
    pub scripts: Vec<InjectedScript>,
}

impl ScriptInjector {
    /// Create an empty injector with the given resource root.
    #[must_use]
    pub fn new(resource_root: PathBuf) -> Self {
        Self {
            resource_root,
            scripts: Vec::new(),
        }
    }

    /// Register a script for injection.
    pub fn register(&mut self, script: InjectedScript) {
        self.scripts.push(script);
    }

    /// Check if a script is registered by ID.
    #[must_use]
    pub fn is_registered(&self, id: &str) -> bool {
        self.scripts.iter().any(|s| s.id == id)
    }

    /// Return all registered script IDs in registration order.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.scripts.iter().map(|s| s.id.as_str()).collect()
    }

    /// Resolve a single script by ID. Returns the resolved source text,
    /// or `None` if no source in the chain resolved.
    #[must_use]
    pub fn resolve(&self, script_id: &str) -> Option<String> {
        let script = self.scripts.iter().find(|s| s.id == script_id)?;
        for source in &script.sources {
            match source {
                ScriptSource::Disk { relative_path } => {
                    let full_path = self.resource_root.join(relative_path);
                    if full_path.exists() && full_path.is_file() {
                        if let Ok(content) = std::fs::read_to_string(&full_path) {
                            return Some(content);
                        }
                        // unreadable → try next source
                    }
                }
                ScriptSource::Static { source } => {
                    return Some((*source).to_string());
                }
            }
        }
        None
    }

    /// Resolve all registered scripts. Returns resolved source texts in
    /// registration order, skipping scripts that fail to resolve entirely.
    #[must_use]
    pub fn resolve_all(&self) -> Vec<String> {
        self.scripts
            .iter()
            .filter_map(|s| self.resolve(&s.id))
            .collect()
    }

    /// Create a `ScriptInjector` pre-loaded with the four standard platform
    /// runtime scripts: scheme interceptor, foundation-wasm, foundation-wasm-ui,
    /// and capability bridge. Each has a disk → static fallback chain.
    #[must_use]
    pub fn with_platform_runtimes(resource_root: PathBuf) -> Self {
        let mut injector = Self::new(resource_root);

        // 1. Platform scheme interceptor
        injector.register(InjectedScript {
            id: "scheme_interceptor".into(),
            sources: vec![
                ScriptSource::Disk {
                    relative_path: PathBuf::from("public/runtimes/platform-scheme-interceptor.js"),
                },
                ScriptSource::Static {
                    source: foundation_wasm_ui::embedded::PLATFORM_SCHEME_INTERCEPTOR_JS,
                },
            ],
        });

        // 2. Foundation WASM core runtime
        injector.register(InjectedScript {
            id: "foundation_wasm".into(),
            sources: vec![
                ScriptSource::Disk {
                    relative_path: PathBuf::from("public/runtimes/foundation-wasm.js"),
                },
                ScriptSource::Static {
                    source: foundation_wasm_ui::embedded::FOUNDATION_WASM_JS,
                },
            ],
        });

        // 3. Foundation WASM UI runtime
        injector.register(InjectedScript {
            id: "foundation_wasm_ui".into(),
            sources: vec![
                ScriptSource::Disk {
                    relative_path: PathBuf::from("public/runtimes/foundation-wasm-ui.js"),
                },
                ScriptSource::Static {
                    source: foundation_wasm_ui::embedded::FOUNDATION_WASM_UI_JS,
                },
            ],
        });

        // 4. Capability bridge (F23)
        injector.register(InjectedScript {
            id: "capability_bridge".into(),
            sources: vec![
                ScriptSource::Disk {
                    relative_path: PathBuf::from("public/runtimes/capability-bridge.js"),
                },
                ScriptSource::Static {
                    source: foundation_wasm_ui::embedded::CAPABILITY_BRIDGE_JS,
                },
            ],
        });

        // 5. Stack viewport — screenshot-swap overlay (F35)
        injector.register(InjectedScript {
            id: "stack_viewport".into(),
            sources: vec![
                ScriptSource::Disk {
                    relative_path: PathBuf::from("public/runtimes/stack-viewport.js"),
                },
                ScriptSource::Static {
                    source: foundation_wasm_ui::embedded::STACK_VIEWPORT_JS,
                },
            ],
        });

        // 6. Floating navigation toolbar (F29 Stage 4)
        injector.register(InjectedScript {
            id: "floating_nav".into(),
            sources: vec![
                ScriptSource::Disk {
                    relative_path: PathBuf::from("public/runtimes/floating-nav.js"),
                },
                ScriptSource::Static {
                    source: foundation_wasm_ui::embedded::FLOATING_NAV_JS,
                },
            ],
        });

        injector
    }
}

// ── Plugin trait for external script registration ──────────────────────

/// A plugin that registers scripts with the [`ScriptInjector`] during setup.
///
/// Capability bridges (F23), IPC bridges (F25), and streaming channel
/// bridges (F26) implement this trait to inject their JS runtime without
/// modifying the bundle generator or HTML templates.
pub trait ScriptInjectorPlugin: Send + Sync + 'static {
    /// Human-readable name (for debugging).
    fn name(&self) -> &str;

    /// Register scripts with the injector.
    fn inject_scripts(&self, injector: &mut ScriptInjector);
}
