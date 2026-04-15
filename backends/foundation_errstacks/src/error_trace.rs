//! # `ErrorTrace<C>` — the core context-aware error trace.
//!
//! **WHY:** Downstream crates want `anyhow`-style ergonomics (a
//! single error type that bubbles up from anywhere) *plus* the
//! compile-time discipline that callers must declare how they
//! interpret a failure when crossing module boundaries. `ErrorTrace`
//! provides that: a typed handle `ErrorTrace<C>` where `C` is the
//! "current context" error type.
//!
//! **WHAT:** This module defines [`ErrorTrace`] and its core
//! constructors (`new`), context-changing (`change_context`),
//! attachment methods (`attach`, `attach_opaque`, and their lazy
//! siblings), and inspection methods (`frames`, `current_context`,
//! `downcast_ref`, `contains`).
//!
//! **HOW:** The trace stores frames in a `Vec<Frame>`; the newest
//! frame is always at the end. Each call to `new` pushes one
//! [`ContextFrame`]; `change_context` pushes another `ContextFrame`
//! and re-tags the type parameter. Attachment methods push
//! [`PrintableFrame`] or [`OpaqueFrame`] frames. The type parameter
//! is stored as `PhantomData<fn() -> *const C>` — a `fn` pointer
//! makes the phantom invariant over `C` without affecting `Send`/`Sync`.

use alloc::vec;
use alloc::vec::Vec;
use core::any::Any;
use core::fmt;
use core::marker::PhantomData;

use crate::frame::{
    AttachmentKind, ContextFrame, Frame, FrameIter, FrameKind, OpaqueFrame, PrintableFrame,
};

/// A structured, context-aware error trace.
///
/// **WHY:** See the module-level docs — `ErrorTrace<C>` is the core
/// value type of this crate. It carries a stack of contexts and
/// attachments describing a failure and lets callers change the
/// interpretive "lens" `C` at module boundaries.
///
/// **WHAT:** A thin handle around a `Vec<Frame>` plus a phantom
/// marker for the current-context type. Construction is via [`new`];
/// see that method and [`change_context`], [`attach`], etc. for the
/// rest of the API.
///
/// **HOW:** Frames are stored in a plain `Vec<Frame>` (clippy's
/// `box_collection` lint forbids `Box<Vec<_>>`; `Vec` already
/// heap-allocates its buffer). The `C` type parameter is phantom-typed
/// to enforce context awareness at compile time.
///
/// When the `backtrace` feature is enabled (requires `std`), each
/// `ErrorTrace` also captures an `std::backtrace::Backtrace` at
/// creation time for enhanced debugging.
///
/// [`new`]: Self::new
/// [`change_context`]: Self::change_context
/// [`attach`]: Self::attach
pub struct ErrorTrace<C: ?Sized> {
    frames: Vec<Frame>,
    _context: PhantomData<fn() -> *const C>,
    #[cfg(feature = "backtrace")]
    backtrace: std::backtrace::Backtrace,
}

// SAFETY: every `FrameImpl` payload is `Send + Sync + 'static`, and
// the phantom marker uses a function pointer so it carries no data.
// `ErrorTrace` therefore inherits `Send + Sync` unconditionally.
unsafe impl<C: ?Sized> Send for ErrorTrace<C> {}
unsafe impl<C: ?Sized> Sync for ErrorTrace<C> {}

impl<C> ErrorTrace<C>
where
    C: core::error::Error + Send + Sync + 'static,
{
    /// WHY: Every error trace starts somewhere — this is the entry
    /// point used by `bail!`, `report!`, and direct construction from
    /// user code.
    ///
    /// WHAT: Creates a new `ErrorTrace<C>` containing a single context
    /// frame wrapping `context`.
    ///
    /// HOW: Allocates a `Vec<Frame>` with one [`ContextFrame`] and
    /// captures the caller location via `#[track_caller]`.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics on its own; allocation failure panics per the
    /// usual global-allocator contract.
    #[track_caller]
    #[must_use]
    pub fn new(context: C) -> Self {
        Self {
            frames: vec![Frame::new(ContextFrame { context })],
            _context: PhantomData,
            #[cfg(feature = "backtrace")]
            backtrace: std::backtrace::Backtrace::capture(),
        }
    }
}

impl<C> ErrorTrace<C>
where
    C: ?Sized,
{
    /// WHY: Callers cross module/crate boundaries and want to
    /// reinterpret a lower-level failure in terms of their own error
    /// vocabulary, while keeping the full history.
    ///
    /// WHAT: Consumes this trace and returns a new `ErrorTrace<T>`
    /// whose top-most frame is a context frame wrapping `context`,
    /// with every prior frame preserved in order.
    ///
    /// HOW: Pushes a new [`ContextFrame`] onto the frame vector and
    /// re-tags the phantom marker. The vector is moved across, not
    /// cloned, so this is allocation-free beyond the new frame.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics on its own; allocation failure panics per the
    /// usual global-allocator contract.
    #[track_caller]
    #[must_use]
    pub fn change_context<T>(self, context: T) -> ErrorTrace<T>
    where
        T: core::error::Error + Send + Sync + 'static,
    {
        let Self { mut frames, .. } = self;
        frames.push(Frame::new(ContextFrame { context }));
        ErrorTrace {
            frames,
            _context: PhantomData,
            #[cfg(feature = "backtrace")]
            backtrace: std::backtrace::Backtrace::capture(),
        }
    }

    /// WHY: Printable attachments are the main way callers enrich an
    /// error trace with human-readable debugging context (request
    /// path, config key, user id, …).
    ///
    /// WHAT: Appends a printable attachment frame to this trace and
    /// returns the updated trace.
    ///
    /// HOW: Wraps `attachment` in a [`PrintableFrame`] and pushes it
    /// onto the frame vector.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics on its own; allocation failure panics per the
    /// usual global-allocator contract.
    #[track_caller]
    #[must_use]
    pub fn attach<A>(mut self, attachment: A) -> Self
    where
        A: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static,
    {
        self.frames.push(Frame::new(PrintableFrame { attachment }));
        self
    }

    /// WHY: Opaque attachments let callers thread structured,
    /// programmatic data through the trace (request metadata, user
    /// context, …) without forcing it into a `Display` impl.
    ///
    /// WHAT: Appends an opaque attachment frame to this trace and
    /// returns the updated trace.
    ///
    /// HOW: Wraps `attachment` in an [`OpaqueFrame`] and pushes it
    /// onto the frame vector.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics on its own; allocation failure panics per the
    /// usual global-allocator contract.
    #[track_caller]
    #[must_use]
    pub fn attach_opaque<A>(mut self, attachment: A) -> Self
    where
        A: Send + Sync + 'static,
    {
        self.frames.push(Frame::new(OpaqueFrame { attachment }));
        self
    }

    /// WHY: On the success path we want `attach` to be free — no
    /// allocation, no formatting cost. The lazy variant defers both
    /// until we already know we have an error.
    ///
    /// WHAT: Same as [`attach`] but accepts a closure that is only
    /// invoked once, when this method is called (on the error path).
    ///
    /// HOW: Calls the closure, wraps the result in a [`PrintableFrame`],
    /// and pushes it.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Propagates any panic from the supplied closure; never panics on
    /// its own beyond that.
    ///
    /// [`attach`]: Self::attach
    #[track_caller]
    #[must_use]
    pub fn attach_with<A, F>(self, f: F) -> Self
    where
        A: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static,
        F: FnOnce() -> A,
    {
        self.attach(f())
    }

    /// WHY: Opaque-attachment counterpart to [`attach_with`] — defers
    /// any construction cost until the error path is actually taken.
    ///
    /// WHAT: Same as [`attach_opaque`] but accepts a closure.
    ///
    /// HOW: Calls the closure, wraps the result in an [`OpaqueFrame`],
    /// and pushes it.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Propagates any panic from the supplied closure; never panics on
    /// its own beyond that.
    ///
    /// [`attach_with`]: Self::attach_with
    /// [`attach_opaque`]: Self::attach_opaque
    #[track_caller]
    #[must_use]
    pub fn attach_opaque_with<A, F>(self, f: F) -> Self
    where
        A: Send + Sync + 'static,
        F: FnOnce() -> A,
    {
        self.attach_opaque(f())
    }

    /// WHY: Formatting, serialization, and diagnostic tooling all
    /// need to walk every frame in the trace.
    ///
    /// WHAT: Returns an iterator over every frame in this trace, in
    /// insertion order (oldest first).
    ///
    /// HOW: Wraps a `core::slice::Iter` over the internal frame
    /// vector.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn frames(&self) -> FrameIter<'_> {
        FrameIter {
            inner: self.frames.iter(),
        }
    }

    /// WHY: Callers frequently need programmatic access to an
    /// attachment or a specific context type — e.g. to branch on
    /// whether a particular cause is present.
    ///
    /// WHAT: Returns a reference to the first frame payload in this
    /// trace whose concrete type is `T`, or `None` if no such frame
    /// exists.
    ///
    /// HOW: Walks [`frames`] in order, calling [`Any::downcast_ref`]
    /// on each frame's type-erased payload.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics.
    ///
    /// [`frames`]: Self::frames
    #[must_use]
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.frames
            .iter()
            .find_map(|frame| frame.as_any().downcast_ref::<T>())
    }

    /// WHY: A common question is "does this trace contain X?" without
    /// actually needing the reference — e.g. for conditional logging.
    ///
    /// WHAT: Returns `true` if any frame in this trace carries a
    /// payload of type `T`.
    ///
    /// HOW: Delegates to [`downcast_ref`].
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics.
    ///
    /// [`downcast_ref`]: Self::downcast_ref
    #[must_use]
    pub fn contains<T>(&self) -> bool
    where
        T: Send + Sync + 'static,
    {
        self.downcast_ref::<T>().is_some()
    }
}

impl<C> ErrorTrace<C>
where
    C: core::error::Error + Send + Sync + 'static,
{
    /// WHY: The whole point of the `C` generic is to let callers
    /// retrieve the current interpretive context, typed.
    ///
    /// WHAT: Returns a reference to the most recently installed
    /// context frame whose concrete type is `C`.
    ///
    /// HOW: Walks [`frames`] in reverse looking for a `Context` frame
    /// that downcasts to `C`.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Panics with a clear message if the invariant is ever violated
    /// — i.e. if an `ErrorTrace<C>` exists without a `C` frame on the
    /// stack. Construction paths (`new`, `change_context`) maintain
    /// this invariant, so in practice this panic is unreachable.
    ///
    /// [`frames`]: Self::frames
    #[must_use]
    pub fn current_context(&self) -> &C {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| match frame.kind() {
                FrameKind::Context(_) => {
                    let any: &dyn Any = frame.as_any();
                    any.downcast_ref::<C>()
                }
                FrameKind::Attachment(_) => None,
            })
            .expect("ErrorTrace<C> invariant violated: no frame of type C present on the stack")
    }
}

// --- Display implementation (Task 2.1) --------------------------------------

impl<C> fmt::Display for ErrorTrace<C>
where
    C: core::error::Error + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Basic format: just show the current context.
        // Alternate format (#): show full trace with all frames.
        if f.alternate() {
            // Full trace format with all frames and locations.
            writeln!(f, "ErrorTrace:")?;
            for (i, frame) in self.frames().enumerate() {
                match frame.kind() {
                    FrameKind::Context(ctx) => {
                        write!(f, "  [{i}] Context: {ctx}")?;
                        if let Some(loc) = frame.location() {
                            write!(f, " (at {loc})")?;
                        }
                        writeln!(f)?;
                    }
                    FrameKind::Attachment(AttachmentKind::Printable(p)) => {
                        write!(f, "  [{i}] Attachment: {p}")?;
                        if let Some(loc) = frame.location() {
                            write!(f, " (at {loc})")?;
                        }
                        writeln!(f)?;
                    }
                    FrameKind::Attachment(AttachmentKind::Opaque(_)) => {
                        write!(f, "  [{i}] Attachment: <opaque>")?;
                        if let Some(loc) = frame.location() {
                            write!(f, " (at {loc})")?;
                        }
                        writeln!(f)?;
                    }
                }
            }
            #[cfg(feature = "backtrace")]
            {
                writeln!(f, "  Backtrace:")?;
                writeln!(f, "{}", self.backtrace)?;
            }
            Ok(())
        } else {
            // Basic format: just the current context message.
            write!(f, "{}", self.current_context())
        }
    }
}

// --- Debug implementation with tree visualization (Task 2.2) ----------------

impl<C> fmt::Debug for ErrorTrace<C>
where
    C: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Tree visualization showing all frames with indentation.
        writeln!(f, "ErrorTrace {{")?;
        writeln!(f, "  frames: [")?;
        for (i, frame) in self.frames().enumerate() {
            write!(f, "    [{i}] ")?;
            match frame.kind() {
                FrameKind::Context(ctx) => {
                    write!(f, "Context({ctx})")?;
                }
                FrameKind::Attachment(AttachmentKind::Printable(p)) => {
                    write!(f, "Printable({p})")?;
                }
                FrameKind::Attachment(AttachmentKind::Opaque(_)) => {
                    write!(f, "Opaque(<any>)")?;
                }
            }
            if let Some(loc) = frame.location() {
                write!(f, " @ {loc}")?;
            }
            writeln!(f, ",")?;
        }
        writeln!(f, "  ]")?;
        write!(f, "}}")
    }
}

// --- Serde support (Task 3.1) -----------------------------------------------

#[cfg(feature = "serde")]
impl<C> serde::Serialize for ErrorTrace<C>
where
    C: core::error::Error + Send + Sync + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("current_context", &self.current_context().to_string())?;
        map.serialize_entry("frames", &self.frames().collect::<Vec<_>>())?;
        map.end()
    }
}

// --- to_structured() method (Task 3.2) --------------------------------------

/// A structured representation of an [`ErrorTrace`] suitable for JSON
/// serialization and logging.
///
/// **WHY:** Downstream services (logging, telemetry, alerting) need a
/// stable, machine-readable format for error traces.
///
/// **WHAT:** A plain struct containing the current context message and
/// a vector of frame representations.
///
/// **HOW:** Built from an `ErrorTrace` by walking its frames and
/// extracting type names, messages, and locations.
#[cfg(feature = "to_structured")]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StructuredErrorTrace {
    /// The current context message (what the error "is" now).
    pub current_context: alloc::string::String,
    /// All frames in the trace, in order from oldest to newest.
    pub frames: alloc::vec::Vec<StructuredFrame>,
    /// Optional backtrace string (only present when `backtrace` feature is on).
    #[cfg(feature = "backtrace")]
    pub backtrace: alloc::string::String,
}

/// A single frame in a [`StructuredErrorTrace`].
#[cfg(feature = "to_structured")]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StructuredFrame {
    /// The kind of frame: "context", "printable", or "opaque".
    pub kind: alloc::string::String,
    /// The message or description for this frame.
    pub message: alloc::string::String,
    /// Optional source location (<file:line:col>).
    pub location: Option<alloc::string::String>,
    /// Optional type name (for opaque frames, this is the concrete type).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub type_name: Option<alloc::string::String>,
}

#[cfg(feature = "to_structured")]
impl<C> ErrorTrace<C>
where
    C: core::error::Error + Send + Sync + 'static,
{
    /// WHY: Callers need a JSON-serializable representation for logging,
    /// telemetry, or API responses.
    ///
    /// WHAT: Converts this `ErrorTrace` into a [`StructuredErrorTrace`]
    /// with all frames expanded into plain data.
    ///
    /// HOW: Walks [`frames()`], extracting kind, message, and location
    /// for each frame.
    ///
    /// [`frames()`]: Self::frames
    #[must_use]
    pub fn to_structured(&self) -> StructuredErrorTrace {
        use alloc::string::ToString;
        use alloc::vec::Vec;
        use core::any::type_name;

        let frames = self
            .frames()
            .map(|frame| {
                let (kind, message, type_name) = match frame.kind() {
                    FrameKind::Context(ctx) => (
                        "context".into(),
                        ctx.to_string(),
                        Some(type_name::<C>().into()),
                    ),
                    FrameKind::Attachment(AttachmentKind::Printable(p)) => {
                        ("printable".into(), p.to_string(), None)
                    }
                    FrameKind::Attachment(AttachmentKind::Opaque(_)) => {
                        ("opaque".into(), "<opaque>".into(), None)
                    }
                };

                let location = frame
                    .location()
                    .map(|loc| alloc::format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));

                StructuredFrame {
                    kind,
                    message,
                    location,
                    type_name,
                }
            })
            .collect::<Vec<_>>();

        StructuredErrorTrace {
            current_context: self.current_context().to_string(),
            #[cfg(feature = "backtrace")]
            backtrace: self.backtrace.to_string(),
            frames,
        }
    }
}

#[cfg(all(feature = "to_structured", feature = "serde"))]
impl StructuredErrorTrace {
    /// Serialize this structured trace to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails (e.g., I/O error from
    /// the serializer).
    pub fn to_json(&self) -> Result<alloc::string::String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

// --- Slack Block Kit formatting (Task 3.3) ----------------------------------

/// A Slack Block Kit block representation.
#[cfg(feature = "slack")]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SlackBlocks {
    /// The blocks array for Slack Block Kit.
    pub blocks: alloc::vec::Vec<SlackBlock>,
}

/// A single Slack Block Kit block.
#[cfg(feature = "slack")]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SlackBlock {
    /// The type of block (e.g., "section", "divider").
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub block_type: alloc::string::String,
    /// Optional text content for the block.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub text: Option<SlackTextObject>,
    /// Optional fields array for multi-column layouts.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub fields: Option<alloc::vec::Vec<SlackTextObject>>,
}

/// A Slack text object.
#[cfg(feature = "slack")]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SlackTextObject {
    /// The type of text object ("mrkdwn" for markdown-style text).
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub text_type: alloc::string::String,
    /// The text content.
    pub text: alloc::string::String,
}

#[cfg(feature = "slack")]
impl StructuredErrorTrace {
    /// Convert this structured trace to Slack Block Kit format.
    ///
    /// # What it produces
    ///
    /// Creates a Slack message with:
    /// - A header section showing the error type
    /// - A divider
    /// - Fields showing each frame in the trace
    /// - An optional backtrace section (when `backtrace` feature is enabled)
    ///
    /// # Example output
    ///
    /// The resulting JSON can be sent to a Slack webhook URL:
    ///
    /// ```json
    /// {
    ///   "blocks": [
    ///     {"type": "section", "text": {"type": "mrkdwn", "text": "*Error: Database connection failed*"}},
    ///     {"type": "divider"},
    ///     {"type": "section", "fields": [{"type": "mrkdwn", "text": "*Frame 0 (context):*\nDatabase connection failed"}]}
    ///   ]
    /// }
    /// ```
    #[must_use]
    pub fn to_slack_blocks(&self) -> SlackBlocks {
        use alloc::format;
        use alloc::vec::Vec;

        let mut blocks: Vec<SlackBlock> = Vec::new();

        // Header section with error type
        blocks.push(SlackBlock {
            block_type: "section".into(),
            text: Some(SlackTextObject {
                text_type: "mrkdwn".into(),
                text: format!("*Error:* {}", self.current_context),
            }),
            fields: None,
        });

        // Divider
        blocks.push(SlackBlock {
            block_type: "divider".into(),
            text: None,
            fields: None,
        });

        // Frame details as fields (grouped for readability)
        let mut fields: Vec<SlackTextObject> = Vec::new();
        for (i, frame) in self.frames.iter().enumerate() {
            let field_text = format!(
                "*Frame {} ({}):*{}\n{}",
                i,
                frame.kind,
                frame
                    .location
                    .as_ref()
                    .map_or_else(alloc::string::String::new, |loc| format!(" _at {loc}_")),
                frame.message
            );
            fields.push(SlackTextObject {
                text_type: "mrkdwn".into(),
                text: field_text,
            });
        }

        if !fields.is_empty() {
            blocks.push(SlackBlock {
                block_type: "section".into(),
                text: None,
                fields: Some(fields),
            });
        }

        // Backtrace section (if available)
        #[cfg(feature = "backtrace")]
        if !self.backtrace.is_empty() {
            blocks.push(SlackBlock {
                block_type: "section".into(),
                text: Some(SlackTextObject {
                    text_type: "mrkdwn".into(),
                    text: format!("*Backtrace:*\n```\n{}\n```", self.backtrace),
                }),
                fields: None,
            });
        }

        SlackBlocks { blocks }
    }

    /// Serialize the Slack blocks to a JSON string suitable for webhook POST.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    #[cfg(feature = "serde")]
    pub fn to_slack_json(&self) -> Result<alloc::string::String, serde_json::Error> {
        let slack_blocks = self.to_slack_blocks();
        serde_json::to_string(&slack_blocks)
    }
}
