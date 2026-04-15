//! # Frame representation for [`crate::ErrorTrace`]
//!
//! **WHY:** An `ErrorTrace` is conceptually a *stack* of things that
//! describe a failure — error contexts that give it meaning plus
//! attachments that enrich it with debugging or programmatic data.
//! We need a single storage shape that can hold any of those kinds
//! uniformly while still letting callers inspect them without losing
//! type information.
//!
//! **WHAT:** This module defines [`Frame`], a type-erased slot for one
//! entry in an error trace; [`FrameKind`] and [`AttachmentKind`],
//! borrowed views used by iteration and formatting; and [`FrameIter`],
//! the iterator returned by [`crate::ErrorTrace::frames`].
//!
//! **HOW:** Each concrete frame payload (a context error, a printable
//! attachment, or an opaque `Any` attachment) is wrapped in a small
//! private struct implementing the crate-private [`FrameImpl`] trait.
//! `Frame` stores that as `Box<dyn FrameImpl>`, plus a `Box<[Frame]>`
//! of child sources reserved for future `caused_by` chains. The
//! iterator walks a `core::slice::Iter`, which terminates cleanly via
//! `None` — satisfying the workspace rule that `Iterator::next` must
//! never use `loop {}`.

use alloc::boxed::Box;
use core::any::Any;
use core::fmt;

/// A single entry in an [`crate::ErrorTrace`].
///
/// **WHY:** Callers of `ErrorTrace` should be able to iterate over
/// every context and attachment in the trace uniformly.
///
/// **WHAT:** A type-erased wrapper around some concrete frame payload,
/// with a (currently empty) slot for child frames reserved for future
/// "caused by" chains.
///
/// **HOW:** Stores a boxed `dyn FrameImpl` trait object plus a boxed
/// slice of source frames.
pub struct Frame {
    inner: Box<dyn FrameImpl>,
    /// Reserved for future support of multi-source "caused by" chains.
    /// Always empty in the current implementation.
    #[allow(dead_code)]
    sources: Box<[Frame]>,
}

impl Frame {
    /// WHY: Internal constructor used by `ErrorTrace` to push a new
    /// frame onto the trace.
    ///
    /// WHAT: Wraps a concrete [`FrameImpl`] in a `Frame`.
    ///
    /// HOW: Boxes the payload and pairs it with an empty sources slice.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics.
    pub(crate) fn new<F: FrameImpl>(frame: F) -> Self {
        Self {
            inner: Box::new(frame),
            sources: Box::new([]),
        }
    }

    /// WHY: Formatters, iterators, and downcasting all need a way to
    /// ask "what kind of frame is this?" without exposing the private
    /// payload types.
    ///
    /// WHAT: Returns a borrowed [`FrameKind`] view of this frame.
    ///
    /// HOW: Delegates to the inner [`FrameImpl`] trait object.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn kind(&self) -> FrameKind<'_> {
        self.inner.kind()
    }

    /// WHY: `ErrorTrace::downcast_ref` needs to inspect each frame to
    /// see whether it carries a specific attachment or context type.
    ///
    /// WHAT: Returns the frame payload as `&dyn Any` for callers that
    /// wish to attempt a downcast.
    ///
    /// HOW: Delegates to the inner [`FrameImpl`] trait object.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn as_any(&self) -> &dyn Any {
        self.inner.as_any()
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            FrameKind::Context(ctx) => f
                .debug_tuple("Context")
                .field(&format_args!("{ctx}"))
                .finish(),
            FrameKind::Attachment(AttachmentKind::Printable(p)) => f
                .debug_tuple("Printable")
                .field(&format_args!("{p}"))
                .finish(),
            FrameKind::Attachment(AttachmentKind::Opaque(_)) => f.debug_tuple("Opaque").finish(),
        }
    }
}

/// A borrowed view of a frame's kind, suitable for iteration and
/// formatting.
pub enum FrameKind<'a> {
    /// The frame holds an error that gives the trace semantic meaning.
    Context(&'a dyn core::error::Error),
    /// The frame holds an attachment — either a printable one or an
    /// opaque `Any` payload.
    Attachment(AttachmentKind<'a>),
}

/// A borrowed view of an attachment frame's payload.
pub enum AttachmentKind<'a> {
    /// A `Display + Debug` attachment suitable for user-visible output.
    Printable(&'a dyn PrintableAttachment),
    /// An opaque attachment accessible only via downcasting.
    Opaque(&'a dyn Any),
}

/// Trait object super-trait uniting `Display`, `Debug`, and `Any` for
/// printable attachments.
///
/// **WHY:** [`AttachmentKind::Printable`] exposes a single borrowed
/// reference that must be usable for both formatting *and*
/// downcasting without the caller juggling multiple trait objects.
///
/// **WHAT:** A single trait implemented for every
/// `T: Display + Debug + Send + Sync + 'static`, allowing
/// `&dyn PrintableAttachment` to stand in for all three capabilities.
///
/// **HOW:** Blanket `impl` below forwards `Display`/`Debug` to the
/// concrete type and exposes `&dyn Any` for downcasting.
pub trait PrintableAttachment: fmt::Display + fmt::Debug + Send + Sync + 'static {
    /// Returns `self` as `&dyn Any` so callers can attempt a downcast.
    ///
    /// # Errors
    /// Never returns an error.
    ///
    /// # Panics
    /// Never panics.
    fn as_any(&self) -> &dyn Any;
}

impl<T> PrintableAttachment for T
where
    T: fmt::Display + fmt::Debug + Send + Sync + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Crate-private trait implemented by every concrete frame payload.
///
/// **WHY:** `Frame` stores its payload type-erased as `Box<dyn
/// FrameImpl>`. This trait is the minimum surface the rest of the
/// crate needs to interact with any frame uniformly.
///
/// **WHAT:** Exposes a kind-view for iteration and an `&dyn Any`
/// accessor for downcasting. Keeping it crate-private prevents
/// downstream users from inventing their own frame types, which lets
/// us evolve the representation freely.
///
/// **HOW:** Implemented by the three private structs in this module
/// (`ContextFrame`, `PrintableFrame`, `OpaqueFrame`).
pub(crate) trait FrameImpl: Send + Sync + 'static {
    fn kind(&self) -> FrameKind<'_>;
    fn as_any(&self) -> &dyn Any;
}

// --- Context frames ---------------------------------------------------------

pub(crate) struct ContextFrame<C: core::error::Error + Send + Sync + 'static> {
    pub(crate) context: C,
}

impl<C> FrameImpl for ContextFrame<C>
where
    C: core::error::Error + Send + Sync + 'static,
{
    fn kind(&self) -> FrameKind<'_> {
        FrameKind::Context(&self.context)
    }

    fn as_any(&self) -> &dyn Any {
        &self.context
    }
}

// --- Printable attachment frames --------------------------------------------

pub(crate) struct PrintableFrame<A: fmt::Display + fmt::Debug + Send + Sync + 'static> {
    pub(crate) attachment: A,
}

impl<A> FrameImpl for PrintableFrame<A>
where
    A: fmt::Display + fmt::Debug + Send + Sync + 'static,
{
    fn kind(&self) -> FrameKind<'_> {
        FrameKind::Attachment(AttachmentKind::Printable(&self.attachment))
    }

    fn as_any(&self) -> &dyn Any {
        &self.attachment
    }
}

// --- Opaque attachment frames -----------------------------------------------

pub(crate) struct OpaqueFrame<A: Send + Sync + 'static> {
    pub(crate) attachment: A,
}

impl<A> FrameImpl for OpaqueFrame<A>
where
    A: Send + Sync + 'static,
{
    fn kind(&self) -> FrameKind<'_> {
        FrameKind::Attachment(AttachmentKind::Opaque(&self.attachment))
    }

    fn as_any(&self) -> &dyn Any {
        &self.attachment
    }
}

// --- Iterator ---------------------------------------------------------------

/// Iterator over the frames of an [`crate::ErrorTrace`].
///
/// **WHY:** Callers need to walk every frame in a trace for formatting,
/// serialization, or downcasting.
///
/// **WHAT:** A zero-cost wrapper over `core::slice::Iter<'a, Frame>`.
///
/// **HOW:** Delegates `next` to the underlying slice iterator, which
/// terminates cleanly via `None` — there is no `loop {}` anywhere in
/// the implementation.
pub struct FrameIter<'a> {
    pub(crate) inner: core::slice::Iter<'a, Frame>,
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = &'a Frame;

    fn next(&mut self) -> Option<Self::Item> {
        // NOTE: delegate to the slice iterator — do NOT introduce a
        // `loop {}` here. The workspace rule against blocking loops in
        // `Iterator::next` is load-bearing for async iterator
        // implementations downstream.
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for FrameIter<'_> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}
