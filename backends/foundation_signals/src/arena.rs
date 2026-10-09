//! WHY: Graph nodes need stable ids that survive removals without dangling — a
//! disposed effect's id must never alias a later node (the dirty heap and
//! observer lists hold ids across mutations). The classic answer is a
//! generational arena; pulling in `slotmap` for ~80 lines isn't worth a new
//! workspace dependency.
//!
//! WHAT: [`NodeId`] (index + generation) and [`Arena`] — insert/get/remove with
//! O(1) everything and ABA protection via generation counters.
//!
//! HOW: A slot vector plus a free list. Removal bumps the slot's generation, so
//! stale `NodeId`s (older generation) simply miss.

use crate::node::Node;

/// Stable identity of a graph node. Copyable, hashable, never dangles —
/// lookups with an id from a removed node return `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

struct Slot {
    generation: u32,
    value: Option<Node>,
}

/// Generational slot arena for [`Node`]s.
#[derive(Default)]
pub(crate) struct Arena {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl Arena {
    pub(crate) fn insert(&mut self, node: Node) -> NodeId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.value = Some(node);
            NodeId {
                index,
                generation: slot.generation,
            }
        } else {
            let index = u32::try_from(self.slots.len()).expect("more than u32::MAX graph nodes");
            self.slots.push(Slot {
                generation: 0,
                value: Some(node),
            });
            NodeId {
                index,
                generation: 0,
            }
        }
    }

    pub(crate) fn get(&self, id: NodeId) -> Option<&Node> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation == id.generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    pub(crate) fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation == id.generation {
            slot.value.as_mut()
        } else {
            None
        }
    }

    /// Remove the node. The slot's generation bumps, so the removed id (and any
    /// copies of it) become permanently stale. Removing twice is a no-op.
    pub(crate) fn remove(&mut self, id: NodeId) -> Option<Node> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation || slot.value.is_none() {
            return None;
        }
        let node = slot.value.take();
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        node
    }

    pub(crate) fn contains(&self, id: NodeId) -> bool {
        self.get(id).is_some()
    }
}
