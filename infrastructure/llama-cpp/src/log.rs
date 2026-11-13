use super::LogOptions;
use std::sync::OnceLock;
use tracing_core::{callsite, field, identify_callsite, Interest, Kind, Metadata};

static FIELD_NAMES: &[&str] = &["message", "module"];

struct OverridableFields {
    message: tracing::field::Field,
    target: tracing::field::Field,
}

macro_rules! log_cs {
    ($level:expr, $cs:ident, $meta:ident, $fields:ident, $ty:ident) => {
        struct $ty;
        static $cs: $ty = $ty;
        static $meta: Metadata<'static> = Metadata::new(
            "log event",
            "llama-cpp-2",
            $level,
            ::core::option::Option::None,
            ::core::option::Option::None,
            ::core::option::Option::None,
            field::FieldSet::new(FIELD_NAMES, identify_callsite!(&$cs)),
            Kind::EVENT,
        );
        static $fields: std::sync::LazyLock<OverridableFields> = std::sync::LazyLock::new(|| {
            let fields = $meta.fields();
            OverridableFields {
                message: fields.field("message").unwrap(),
                target: fields.field("module").unwrap(),
            }
        });

        impl callsite::Callsite for $ty {
            fn set_interest(&self, _: Interest) {}
            fn metadata(&self) -> &'static Metadata<'static> {
                &$meta
            }
        }
    };
}
log_cs!(
    tracing_core::Level::TRACE,
    TRACE_CS,
    TRACE_META,
    TRACE_FIELDS,
    TraceCallsite
);
log_cs!(
    tracing_core::Level::DEBUG,
    DEBUG_CS,
    DEBUG_META,
    DEBUG_FIELDS,
    DebugCallsite
);
// No INFO callsite: nothing maps to tracing INFO any more. See
// `tracing_level_for` — llama.cpp's INFO is a load-time dump and is emitted at
// DEBUG so a plain `info` filter stays usable without per-app directives.
log_cs!(
    tracing_core::Level::WARN,
    WARN_CS,
    WARN_META,
    WARN_FIELDS,
    WarnCallsite
);
log_cs!(
    tracing_core::Level::ERROR,
    ERROR_CS,
    ERROR_META,
    ERROR_FIELDS,
    ErrorCallsite
);

/// The native library that produced a log line.
#[derive(Clone, Copy)]
pub enum Module {
    /// The `ggml` tensor library.
    GGML,
    /// The `llama.cpp` model library.
    LlamaCpp,
}

impl Module {
    const fn name(&self) -> &'static str {
        match self {
            Module::GGML => "ggml",
            Module::LlamaCpp => "llama.cpp",
        }
    }
}

/// Map a ggml log level onto the tracing level the event is emitted at.
///
/// WHY the levels are shifted down rather than passed through: llama.cpp's
/// notion of INFO is a load-time diagnostic dump — 165 lines of tensor and
/// vocabulary metadata on every single model load — not the handful of
/// application-level events a Rust caller means by `info`. Passing it through
/// meant every binary embedding this crate had to carry a `llama-cpp-2=off`
/// directive just to make `RUST_LOG=info` usable, and anyone who forgot got a
/// wall of C output.
///
/// WARN is also demoted to DEBUG for the same reason: the warnings llama.cpp
/// emits (token-type mismatches, vocabulary quirks) are model-metadata noise,
/// not something the end user can act on. A plain `info` filter should be
/// usable without any per-target directives.
///
/// ERROR stays at ERROR: when llama.cpp has a real fault, the caller must hear
/// about it even under a quiet filter.
///
/// WHAT: ggml DEBUG → TRACE, ggml INFO/WARN → DEBUG, ggml ERROR → ERROR.
#[must_use]
pub const fn tracing_level_for(
    level: infrastructure_llama_bindings::ggml_log_level,
) -> tracing_core::Level {
    match level {
        infrastructure_llama_bindings::GGML_LOG_LEVEL_DEBUG => tracing_core::Level::TRACE,
        infrastructure_llama_bindings::GGML_LOG_LEVEL_INFO
        | infrastructure_llama_bindings::GGML_LOG_LEVEL_WARN => tracing_core::Level::DEBUG,
        _ => tracing_core::Level::ERROR,
    }
}

/// Pick the callsite for a ggml level, via [`tracing_level_for`].
///
/// Routed through that function rather than repeating the mapping, so the
/// levels the unit tests assert on are the levels events are actually emitted
/// at. Duplicating it meant `tracing_level_for` was dead code the tests still
/// happily covered — a reverted mapping here would not have failed anything.
fn meta_for_level(
    level: infrastructure_llama_bindings::ggml_log_level,
) -> (&'static Metadata<'static>, &'static OverridableFields) {
    match tracing_level_for(level) {
        tracing_core::Level::TRACE => (&TRACE_META, &TRACE_FIELDS),
        tracing_core::Level::DEBUG => (&DEBUG_META, &DEBUG_FIELDS),
        tracing_core::Level::WARN => (&WARN_META, &WARN_FIELDS),
        _ => (&ERROR_META, &ERROR_FIELDS),
    }
}

/// Buffered state that `logs_to_trace` reads for one native module.
pub struct State {
    pub(super) options: LogOptions,
    module: Module,
    buffered: std::sync::Mutex<Option<(infrastructure_llama_bindings::ggml_log_level, String)>>,
    previous_level: std::sync::atomic::AtomicI32,
    is_buffering: std::sync::atomic::AtomicBool,
}

impl State {
    /// Create the buffering state for one native module.
    #[must_use]
    pub fn new(module: Module, options: LogOptions) -> Self {
        Self {
            options,
            module,
            buffered: Default::default(),
            previous_level: Default::default(),
            is_buffering: Default::default(),
        }
    }

    fn generate_log(
        target: Module,
        level: infrastructure_llama_bindings::ggml_log_level,
        text: &str,
    ) {
        // Annoying but tracing requires that the provided target name is a string literal and
        // even &'static str isn't enough so we have to duplicate the generation AND we can't even
        // extract the interrior module within llama.cpp/ggml to be able to propagate it forward.
        // This happens because the target is part of a static variable injected by the macro that's
        // initialized with said target.

        let (module, text) = text
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_lowercase() || *c == '_')
            .last()
            .and_then(|(pos, _)| {
                let next_two = text.get(pos + 1..pos + 3);
                if next_two == Some(": ") {
                    let (sub_module, text) = text.split_at(pos + 1);
                    let text = text.split_at(2).1;
                    Some((Some(format!("{}::{sub_module}", target.name())), text))
                } else {
                    None
                }
            })
            .unwrap_or((None, text));

        let (meta, fields) = meta_for_level(level);

        tracing::dispatcher::get_default(|dispatcher| {
            dispatcher.event(&tracing::Event::new(
                meta,
                &meta.fields().value_set(&[
                    (&fields.message, Some(&text as &dyn tracing::field::Value)),
                    (
                        &fields.target,
                        module.as_ref().map(|s| s as &dyn tracing::field::Value),
                    ),
                ]),
            ));
        });
    }

    /// Append more text to the previously buffered log. The text may or may not end with a newline.
    pub(super) fn cont_buffered_log(&self, text: &str) {
        let mut lock = self.buffered.lock().unwrap();

        if let Some((previous_log_level, mut buffer)) = lock.take() {
            buffer.push_str(text);
            if buffer.ends_with('\n') {
                self.is_buffering
                    .store(false, std::sync::atomic::Ordering::Release);
                Self::generate_log(self.module, previous_log_level, buffer.as_str());
            } else {
                *lock = Some((previous_log_level, buffer));
            }
        } else {
            let level = self
                .previous_level
                .load(std::sync::atomic::Ordering::Acquire)
                as infrastructure_llama_bindings::ggml_log_level;
            tracing::warn!(
                inferred_level = level,
                text = text,
                origin = "crate",
                "llma.cpp sent out a CONT log without any previously buffered message"
            );
            *lock = Some((level, text.to_string()));
        }
    }

    /// Start buffering a message. Not the CONT log level and text is missing a newline.
    pub(super) fn buffer_non_cont(
        &self,
        level: infrastructure_llama_bindings::ggml_log_level,
        text: &str,
    ) {
        debug_assert!(!text.ends_with('\n'));
        debug_assert_ne!(level, infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT);

        if let Some((previous_log_level, buffer)) = self
            .buffered
            .lock()
            .unwrap()
            .replace((level, text.to_string()))
        {
            tracing::warn!(
                level = previous_log_level,
                text = &buffer,
                origin = "crate",
                "Message buffered unnnecessarily due to missing newline and not followed by a CONT"
            );
            Self::generate_log(self.module, previous_log_level, buffer.as_str());
        }

        self.is_buffering
            .store(true, std::sync::atomic::Ordering::Release);
        self.previous_level
            .store(level as i32, std::sync::atomic::Ordering::Release);
    }

    // Emit a normal unbuffered log message (not the CONT log level and the text ends with a newline).
    pub(super) fn emit_non_cont_line(
        &self,
        level: infrastructure_llama_bindings::ggml_log_level,
        text: &str,
    ) {
        debug_assert!(text.ends_with('\n'));
        debug_assert_ne!(level, infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT);

        if self
            .is_buffering
            .swap(false, std::sync::atomic::Ordering::Acquire)
        {
            if let Some((buf_level, buf_text)) = self.buffered.lock().unwrap().take() {
                // This warning indicates a bug within llama.cpp
                tracing::warn!(level = buf_level, text = buf_text, origin = "crate", "llama.cpp message buffered spuriously due to missing \\n and being followed by a non-CONT message!");
                Self::generate_log(self.module, buf_level, buf_text.as_str());
            }
        }

        self.previous_level
            .store(level as i32, std::sync::atomic::Ordering::Release);

        let (text, newline) = text.split_at(text.len() - 1);
        debug_assert_eq!(newline, "\n");

        match level {
            infrastructure_llama_bindings::GGML_LOG_LEVEL_NONE => {
                // TODO: Support logging this to stdout directly via options?
                tracing::info!(no_log_level = true, text);
            }
            infrastructure_llama_bindings::GGML_LOG_LEVEL_DEBUG
            | infrastructure_llama_bindings::GGML_LOG_LEVEL_INFO
            | infrastructure_llama_bindings::GGML_LOG_LEVEL_WARN
            | infrastructure_llama_bindings::GGML_LOG_LEVEL_ERROR => {
                Self::generate_log(self.module, level, text);
            }
            infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT => unreachable!(),
            _ => {
                tracing::warn!(
                    level = level,
                    text = text,
                    origin = "crate",
                    "Unknown llama.cpp log level"
                );
            }
        }
    }

    pub(super) fn update_previous_level_for_disabled_log(
        &self,
        level: infrastructure_llama_bindings::ggml_log_level,
    ) {
        if level != infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT {
            self.previous_level
                .store(level as i32, std::sync::atomic::Ordering::Release);
        }
    }

    /// Checks whether the given log level is enabled by the current tracing subscriber.
    pub(super) fn is_enabled_for_level(
        &self,
        level: infrastructure_llama_bindings::ggml_log_level,
    ) -> bool {
        // CONT logs do not need to check if they are enabled.
        let level = if level == infrastructure_llama_bindings::GGML_LOG_LEVEL_CONT {
            self.previous_level
                .load(std::sync::atomic::Ordering::Relaxed)
                as infrastructure_llama_bindings::ggml_log_level
        } else {
            level
        };
        let (meta, _) = meta_for_level(level);
        tracing::dispatcher::get_default(|dispatcher| dispatcher.enabled(meta))
    }
}

pub(super) static LLAMA_STATE: OnceLock<Box<State>> = OnceLock::new();
pub(super) static GGML_STATE: OnceLock<Box<State>> = OnceLock::new();
