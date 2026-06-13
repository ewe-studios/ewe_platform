//! # M1 — Anchored positioning (stub)
//!
//! Placeholder for the positioning module. The full M1 spec covers:
//! - Side/align placement (top/bottom/left/right + start/end)
//! - Collision handling (flip/shift/none for side and align axes)
//! - Fallback axis (start/end/none)
//! - CSS variable emission (--anchor-width, --available-height, etc.)
//! - data-side/data-align attributes for CSS arrow positioning
//!
//! For now, this module defines the types that overlay components will use.
//! The actual JS-side positioning logic (spec-42 feature 05 §M1) is
//! deferred to the machinery implementation phase.

/// Placement side relative to the anchor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlacementSide {
    /// Above the anchor.
    #[default]
    Top,
    /// Below the anchor.
    Bottom,
    /// To the left of the anchor.
    Left,
    /// To the right of the anchor.
    Right,
}

impl PlacementSide {
    /// The `primal:side` token consumed by the M1 positioner.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PlacementSide::Top => "top",
            PlacementSide::Bottom => "bottom",
            PlacementSide::Left => "left",
            PlacementSide::Right => "right",
        }
    }
}

/// Alignment along the anchor's cross axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlacementAlign {
    /// Center-aligned.
    #[default]
    Center,
    /// Aligned to the start edge.
    Start,
    /// Aligned to the end edge.
    End,
}

impl PlacementAlign {
    /// The `primal:align` token consumed by the M1 positioner.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PlacementAlign::Center => "center",
            PlacementAlign::Start => "start",
            PlacementAlign::End => "end",
        }
    }
}

/// Collision behavior for a single axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollisionBehavior {
    /// Flip to the opposite side/align when overflowing.
    #[default]
    Flip,
    /// Shift along the axis to stay visible.
    Shift,
    /// No correction.
    None,
}

impl CollisionBehavior {
    /// The `data-collision-side`/`data-collision-align` token consumed by the
    /// M1 positioner.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            CollisionBehavior::Flip => "flip",
            CollisionBehavior::Shift => "shift",
            CollisionBehavior::None => "none",
        }
    }
}

/// Positioning configuration for overlay components.
pub struct PositionConfig {
    /// Preferred placement side.
    pub side: PlacementSide,
    /// Alignment along the cross axis.
    pub align: PlacementAlign,
    /// Offset from the anchor in pixels.
    pub offset: i32,
    /// Collision behavior for the side axis.
    pub collision_side: CollisionBehavior,
    /// Collision behavior for the align axis.
    pub collision_align: CollisionBehavior,
    /// Fallback axis direction when preferred axis cannot fit.
    pub fallback_axis: Option<PlacementSide>,
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            side: PlacementSide::Bottom,
            align: PlacementAlign::Center,
            offset: 0,
            collision_side: CollisionBehavior::Flip,
            collision_align: CollisionBehavior::Flip,
            fallback_axis: None,
        }
    }
}

impl PositionConfig {
    /// Popover default: bottom-center with flip on both axes.
    #[must_use]
    pub fn popover() -> Self {
        Self {
            side: PlacementSide::Bottom,
            align: PlacementAlign::Center,
            offset: 4,
            collision_side: CollisionBehavior::Flip,
            collision_align: CollisionBehavior::Flip,
            fallback_axis: None,
        }
    }

    /// Tooltip default: top-center with flip.
    #[must_use]
    pub fn tooltip() -> Self {
        Self {
            side: PlacementSide::Top,
            align: PlacementAlign::Center,
            offset: 4,
            collision_side: CollisionBehavior::Flip,
            collision_align: CollisionBehavior::Flip,
            fallback_axis: None,
        }
    }
}
