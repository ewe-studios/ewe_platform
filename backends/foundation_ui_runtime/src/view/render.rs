use foundation_ui_traits::{AttrName, Html};
use foundation_ui_wire::{PlaceT, SelectorKind, TargetSelectorT};
use serde::Serialize;
use std::sync::{Arc, Mutex};

use super::lower::{append, attribute, build, Layout, Tree};
use super::{LowerError, LowerOptions, PlacementKind, TokenTable};
use crate::action::{ActionReceiver, ActionResult, Context};

/// Allocates a contiguous range of identifiers in one mounted view.
pub trait NodeIdAllocator: Send + Sync {
    /// Returns the first identifier in a fresh range with the given size and minimum.
    ///
    /// # Errors
    /// Returns an error if the range cannot fit in the view.
    fn reserve(&self, minimum: u32, count: u32) -> Result<u32, LowerError>;
}

struct Sequence {
    next: Mutex<u64>,
    last: u32,
}

impl NodeIdAllocator for Sequence {
    fn reserve(&self, minimum: u32, count: u32) -> Result<u32, LowerError> {
        let mut next = self
            .next
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let first = (*next).max(u64::from(minimum));
        let end = first + u64::from(count);
        if first > u64::from(u32::MAX) || end > u64::from(self.last) + 1 {
            return Err(LowerError::IdOverflow);
        }
        *next = end;
        u32::try_from(first).map_err(|_| LowerError::IdOverflow)
    }
}

/// The allocator shared by producers that write into one mounted view.
#[derive(Clone)]
pub struct ViewIds(Arc<dyn NodeIdAllocator>);

impl ViewIds {
    /// Starts a monotonically increasing range at `first`.
    #[must_use]
    pub fn new(first: u32) -> Self {
        Self::range(first, u32::MAX)
    }

    /// Reserves identifiers within the inclusive bounds that the host assigns to this producer.
    #[must_use]
    pub fn range(first: u32, last: u32) -> Self {
        Self(Arc::new(Sequence {
            next: Mutex::new(u64::from(first)),
            last,
        }))
    }

    /// Uses a host allocator, for example one shared by several server replicas.
    #[must_use]
    pub fn with_allocator(allocator: Arc<dyn NodeIdAllocator>) -> Self {
        Self(allocator)
    }
}

impl Default for ViewIds {
    fn default() -> Self {
        Self::new(1)
    }
}

/// A mounted node or a selector understood by a web renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Node(u32),
    Css(String),
}

impl From<u32> for Target {
    fn from(id: u32) -> Self {
        Self::Node(id)
    }
}
impl From<String> for Target {
    fn from(selector: String) -> Self {
        Self::Css(selector)
    }
}
impl From<&str> for Target {
    fn from(selector: &str) -> Self {
        Self::Css(selector.into())
    }
}
impl From<&String> for Target {
    fn from(selector: &String) -> Self {
        Self::Css(selector.clone())
    }
}

impl Target {
    fn id(&self) -> Option<u32> {
        match self {
            Self::Node(id) => Some(*id),
            Self::Css(_) => None,
        }
    }

    fn wire(self) -> Result<Box<TargetSelectorT>, LowerError> {
        let selector = match self {
            Self::Node(id) => table!(TargetSelectorT {
                kind: SelectorKind::NodeId,
                node_id: id
            }),
            Self::Css(css) if !css.trim().is_empty() => table!(TargetSelectorT {
                kind: SelectorKind::Query,
                text: Some(css)
            }),
            Self::Css(_) => return Err(LowerError::BadTarget),
        };
        Ok(Box::new(selector))
    }
}

fn frame_tree(
    ctx: &Context,
    html: &Html,
    minimum: u32,
    root: Option<u32>,
) -> Result<Tree, LowerError> {
    let layout = Layout::new(html)?;
    let count = layout.span()?;
    let minimum = minimum.max(layout.minimum()?);
    let minimum = if root == Some(minimum) && super::lower::local_id(html)? != 0 {
        minimum.checked_add(1).ok_or(LowerError::IdOverflow)?
    } else {
        minimum
    };
    let ids = ctx
        .get::<ViewIds>()
        .unwrap_or_else(|| ctx.get_or_insert(ViewIds::default()));
    let base = ids.0.reserve(minimum, count)?;
    if base < minimum || base.checked_add(count - 1).is_none() {
        return Err(LowerError::IdOverflow);
    }
    let tokens = ctx.get::<TokenTable>();
    let empty = TokenTable::default();
    build(
        html,
        &LowerOptions {
            id_base: base,
            tokens: tokens.as_deref().unwrap_or(&empty),
        },
        &layout,
        root,
    )
}

fn queue<S: Serialize + ?Sized>(
    out: &ActionReceiver,
    mut tree: Tree,
    placement: Option<PlaceT>,
    state: Option<&S>,
) -> ActionResult {
    if state.is_none() && !tree.markers.is_empty() {
        return Err(LowerError::MissingState.into());
    }
    tree.ops.push(attribute(
        tree.root,
        &AttrName::from_static("data-ewe-root"),
        "",
    ));
    let mut targets = vec![tree.root];
    targets.extend(tree.markers);
    out.render_tree(tree.ops, placement, &targets, state)
}

/// Queues a complete view for the mount that receives this frame.
///
/// # Errors
/// Returns a lowering, allocation or state serialization error.
pub fn full_view<S: Serialize + ?Sized>(
    ctx: &Context,
    out: &ActionReceiver,
    html: &Html,
    state: Option<&S>,
) -> ActionResult {
    let tree = frame_tree(ctx, html, 3, None)?;
    let placement = table!(PlaceT {
        kind: PlacementKind::FullView,
        root_node: tree.root
    });
    queue(out, tree, Some(placement), state)
}

/// Builds an independent tree below a mounted parent.
///
/// # Errors
/// Returns a lowering or state serialization error before queuing any output.
pub fn mount<S: Serialize + ?Sized>(
    html: &Html,
    options: &LowerOptions<'_>,
    parent: u32,
    out: &ActionReceiver,
    state: Option<&S>,
) -> ActionResult {
    let mut tree = build(html, options, &Layout::new(html)?, None)?;
    tree.ops.push(append(parent, tree.root));
    queue(out, tree, None, state)
}

/// Renders an initial component or replaces the call's trusted component root.
///
/// # Errors
/// Returns a lowering, identifier allocation or state serialization error.
pub fn render<S: Serialize + ?Sized>(
    ctx: &Context,
    out: &ActionReceiver,
    mut html: Html,
    state: Option<&S>,
) -> Result<Html, foundation_errstacks::ErrorTrace<crate::action::ActionError>> {
    let root = ctx.state_root();
    if ctx.is_action() && root.is_none() {
        return Err(LowerError::NoComponentRoot.into());
    }
    let tree = frame_tree(ctx, &html, root.unwrap_or(1), root)?;
    html.runtime_id = Some(tree.root);
    let placement = if let Some(root) = root {
        Some(table!(PlaceT {
            kind: PlacementKind::Swap,
            target: Some(Target::Node(root).wire()?),
            root_node: root
        }))
    } else {
        None
    };
    queue(out, tree, placement, state)?;
    Ok(html)
}

/// Queues one complete placement. The renderer resolves the target before it applies the nested operations.
///
/// # Errors
/// Returns an error for invalid targets, missing content, invalid templates or unserializable state.
pub fn place<S: Serialize + ?Sized>(
    ctx: &Context,
    out: &ActionReceiver,
    kind: PlacementKind,
    target: Target,
    child: Option<Target>,
    html: Option<Html>,
    state: Option<&S>,
) -> ActionResult {
    let minimum = target
        .id()
        .into_iter()
        .chain(child.as_ref().and_then(Target::id))
        .max()
        .unwrap_or(0);
    if child.is_some() && !matches!(kind, PlacementKind::Before | PlacementKind::After) {
        return Err(LowerError::BadTarget.into());
    }
    let mut placement = table!(PlaceT {
        kind: kind,
        target: Some(target.wire()?),
        child: child.map(Target::wire).transpose()?
    });
    if kind == PlacementKind::Remove {
        if html.is_some() || state.is_some() {
            return Err(LowerError::BadTarget.into());
        }
        return out.render_tree(Vec::new(), Some(placement), &[], None::<&()>);
    }
    if !matches!(
        kind,
        PlacementKind::Append | PlacementKind::Before | PlacementKind::After | PlacementKind::Swap
    ) {
        return Err(LowerError::BadTarget.into());
    }
    let html = html.ok_or(LowerError::NoRootElement)?;
    let tree = frame_tree(
        ctx,
        &html,
        minimum.checked_add(1).ok_or(LowerError::IdOverflow)?,
        None,
    )?;
    placement.root_node = tree.root;
    queue(out, tree, Some(placement), state)
}
