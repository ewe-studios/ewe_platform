//! Track local reactive scopes by the roots that an instance mounted.
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use foundation_signals::Context;

type Owners = BTreeMap<u32, Vec<Rc<Owner>>>;

#[derive(Clone, Default)]
pub(crate) struct Mounts(Rc<RefCell<Owners>>);

struct Owner {
    roots_left: Cell<usize>,
    dispose: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl Mounts {
    pub(crate) fn register(&self, roots: &[u32], scope: &Context) {
        let ids: BTreeSet<_> = roots.iter().copied().collect();
        if ids.is_empty() {
            return;
        }
        let owner = Rc::new(Owner {
            roots_left: Cell::new(ids.len()),
            dispose: RefCell::new(Some(Box::new(scope.disposer()))),
        });
        for id in &ids {
            self.0
                .borrow_mut()
                .entry(*id)
                .or_default()
                .push(Rc::clone(&owner));
        }
        let mounts = Rc::downgrade(&self.0);
        scope.on_dispose(move || {
            if let Some(mounts) = mounts.upgrade() {
                let mut table = mounts.borrow_mut();
                for id in ids {
                    if let Some(owners) = table.get_mut(&id) {
                        owners.retain(|other| !Rc::ptr_eq(other, &owner));
                        if owners.is_empty() {
                            table.remove(&id);
                        }
                    }
                }
            }
        });
    }

    pub(crate) fn contains(&self, id: u32) -> bool {
        self.0.borrow().contains_key(&id)
    }

    pub(crate) fn release(&self, id: u32) {
        let owners = self.0.borrow_mut().remove(&id).unwrap_or_default();
        for owner in owners {
            let left = owner.roots_left.get() - 1;
            owner.roots_left.set(left);
            if left == 0 {
                let dispose = owner.dispose.borrow_mut().take();
                if let Some(dispose) = dispose {
                    dispose();
                }
            }
        }
    }
}
