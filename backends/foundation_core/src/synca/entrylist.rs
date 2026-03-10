use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use foundation_nostd::comp::basic::RwLock;

#[cfg(target_arch = "wasm32")]
pub use foundation_nostd::primtivies::RwLock;

/// Entry based list using generation markers to identify
/// used list items in an efficient list.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Entry {
    pub id: usize,
    pub gen: usize,
}

#[allow(dead_code)]
impl Entry {
    #[must_use]
    pub fn new(id: usize, gen: usize) -> Self {
        Self { id, gen }
    }
}

#[derive(Debug, Clone)]
pub struct EntryList<T> {
    items: Vec<(usize, Option<T>)>,
    free_entries: Vec<Entry>,
    packed_entries: Vec<Entry>,
}

// --- constructors

impl<T> EntryList<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            free_entries: Vec::new(),
            packed_entries: Vec::new(),
        }
    }
}

impl<T> Default for EntryList<T> {
    fn default() -> Self {
        Self::new()
    }
}

// --- methods

impl<T> EntryList<T> {
    /// `active_slots` returns how many slots have value and are in use.
    ///
    /// Basically does a calculation using:
    /// `EntryList::allocated_slots()` - `EntryList::active_slots()`.
    ///
    /// Returning the difference indicative of which slots do have value
    /// actively in use and not just empty and available for re-allocation.
    #[inline]
    #[must_use]
    pub fn active_slots(&self) -> usize {
        self.allocated_slots() - self.open_slots()
    }

    /// Returns total entries currently parked.
    #[inline]
    #[must_use]
    pub fn parked_slots(&self) -> usize {
        self.packed_entries.len()
    }

    /// `allocated_slots` returns how many slots have being allocated overall.
    #[inline]
    #[must_use]
    pub fn allocated_slots(&self) -> usize {
        self.items.len()
    }

    /// `open_slots` returns how many free entries are now available.
    #[inline]
    #[must_use]
    pub fn open_slots(&self) -> usize {
        self.free_entries.len()
    }

    /// `get_mut` lets you perform an in-place value replacement without
    /// invalidating the `Entry` handle you have
    /// pointing to the given value it points to.
    ///
    /// This is useful in those cases where all you really wish to do is
    /// update the underlying value without loosing your key like you would
    /// a regular map.
    #[inline]
    pub fn get_mut(&mut self, entry: &Entry) -> Option<&mut T> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen && value.is_some() {
                return value.as_mut();
            }
        }
        None
    }

    /// get a reference to the relevant value within the
    /// list for the giving `Entry` if its still valid.
    #[inline]
    #[must_use]
    pub fn get(&self, entry: &Entry) -> Option<&T> {
        if let Some((gen, value)) = self.items.get(entry.id) {
            if *gen == entry.gen && value.is_some() {
                return value.as_ref();
            }
        }
        None
    }

    /// `not_valid` returns bool (True/False) indicating if the entry
    /// reference is still valid.
    #[inline]
    pub fn not_valid(&mut self, entry: &Entry) -> bool {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen != entry.gen {
                return true;
            }

            if *gen == entry.gen && value.is_none() {
                return true;
            }
        }
        false
    }

    /// has returns bool (True/False) indicating if the entry
    /// exists and is still valid.
    #[inline]
    pub fn has(&mut self, entry: &Entry) -> bool {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen && value.is_some() {
                return true;
            }
        }
        false
    }

    /// vacate eats the value at that location in the list
    /// freeing the entry for re-use if not already.
    ///
    /// The old value is dropped if it indeed is valid/has-value.
    #[inline]
    pub fn vacate(&mut self, entry: &Entry) {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                if let Some(con) = value.take() {
                    self.free_entries.push(*entry);
                    drop(con);
                }
            }
        }
    }

    /// pack collects the value pointed to by the relevant
    /// `Entry` pointer if its still valid but does not invalidate
    /// the pointer.
    /// You can think of this as a temporary borrow where we want
    /// to borrow that given entry, use it for some undefined period of
    /// time and be guaranteed that slot will not be usable till it's unpacked
    /// basically, you own that entry till its unpacked.
    ///
    /// This allows us support situations where we need to maintain that entry
    /// but cant afford to invalid the entry due to dependency chains built on it.
    #[inline]
    pub fn park(&mut self, entry: &Entry) -> Option<T> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                if let Some(con) = value.take() {
                    self.packed_entries.push(*entry);
                    return Some(con);
                }
            }
        }
        None
    }

    /// unpack helps you to re-allocate the provided value
    /// back into the packed entry, if the entry was truly
    /// packed then true is returned to validate that
    /// the entry was indeed found and updated.
    pub fn unpark(&mut self, entry: &Entry, item: T) -> bool {
        match self.find_packed(entry) {
            Some(index) => {
                self.packed_entries.remove(index);
                let _ = self.update_packed(entry, item);
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn find_packed(&self, entry: &Entry) -> Option<usize> {
        for (index, item) in self.packed_entries.iter().enumerate() {
            if item == entry {
                return Some(index);
            }
        }
        None
    }

    #[inline]
    pub fn update_packed(&mut self, entry: &Entry, item: T) -> Option<T> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen && value.is_none() {
                // collect old value
                let previous_value = value.take();

                // replace value
                *value = Some(item);

                // Return new Entry and Old value.
                return previous_value;
            }
        }
        None
    }

    /// take collects the value pointed to by the relevant
    /// `Entry` pointer if its still valid and then invalidates
    /// the pointer.
    #[inline]
    pub fn take(&mut self, entry: &Entry) -> Option<T> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                if let Some(con) = value.take() {
                    self.free_entries.push(*entry);
                    return Some(con);
                }
            }
        }
        None
    }

    /// update lets you change the underlying data for a giving reference
    /// without invalidating the reference for that object
    /// and returning old value.
    #[inline]
    pub fn update(&mut self, entry: &Entry, item: T) -> Option<T> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen && value.is_some() {
                // collect old value
                let previous_value = value.take();

                // replace value
                *value = Some(item);

                // Return new Entry and Old value.
                return previous_value;
            }
        }
        None
    }

    /// `for_each` loop through all active entries.
    #[inline]
    pub fn map_with<V>(&self, tn: impl Fn(&T) -> Option<V>) -> Vec<V> {
        self.items
            .iter()
            .filter_map(|(_gen, value)| -> Option<V> {
                if value.is_none() {
                    return None;
                }

                match value {
                    Some(item) => tn(item),
                    None => None,
                }
            })
            .collect()
    }

    /// `for_each` loop through all active entries.
    #[inline]
    pub fn for_each(&self, tn: impl Fn(Option<&T>)) {
        self.items.iter().for_each(|(_gen, value)| {
            if value.is_none() {
                return;
            }

            tn(value.as_ref());
        });
    }

    /// `select_take` loop through all active entries, using the provided
    /// function as a filter and takes the relevant matching values
    /// out of the list returned as an `Vec<T>`.
    ///
    /// This becomes heavly useful when you wish to take a series of underlying
    /// values that match a condition and vacate the underlying entries to be available
    /// for reuse.
    #[inline]
    pub fn select_take(&mut self, tn: impl Fn(&T) -> bool) -> Vec<T> {
        self.items
            .iter_mut()
            .enumerate()
            .filter(|(_index, (_gen, value))| match value {
                Some(inner) => tn(inner),
                None => false,
            })
            .map(|(index, (gen, value))| {
                self.free_entries.push(Entry::new(index, *gen));
                value.take().unwrap()
            })
            .collect()
    }

    /// replace lets you change the underlying data for a giving reference
    /// invalidating the previous reference for that object
    /// and returning the new reference and old value.
    #[inline]
    pub fn replace(&mut self, entry: &Entry, item: T) -> Option<(Entry, T)> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                let new_gen = *gen + 1;
                if value.is_some() {
                    // collect old value
                    let previous_value = value.take();
                    tracing::debug!(
                        "Current entry with gen: {} going to new gen: {} for id: {}",
                        gen,
                        new_gen,
                        entry.id,
                    );

                    // replace gen
                    *gen = new_gen;
                    tracing::debug!("Update gen to: {} for id: {}", new_gen, entry.id);

                    // replace value
                    *value = Some(item);

                    // Return new Entry and Old value.
                    return Some((
                        Entry {
                            gen: new_gen,
                            id: entry.id,
                        },
                        previous_value.unwrap(),
                    ));
                }
            }
        }
        None
    }

    /// inserts a new value into the list receiving the relevant
    /// `Entry` handle for the item.
    #[inline]
    pub fn insert(&mut self, item: T) -> Entry {
        let entry = match self.free_entries.pop() {
            Some(mut inner) => {
                inner.gen += 1;
                inner
            }
            None => Entry {
                id: self.items.len(),
                gen: 0,
            },
        };

        if self.items.len() == entry.id {
            self.items.push((entry.gen, Some(item)));
        } else {
            self.items[entry.id] = (entry.gen, Some(item));
        }

        entry
    }
}

pub struct ThreadSafeEntry<T>(Arc<RwLock<EntryList<T>>>);

impl<T> Default for ThreadSafeEntry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ThreadSafeEntry<T> {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(EntryList::new())))
    }

    #[must_use]
    pub fn from(list: EntryList<T>) -> Self {
        Self(Arc::new(RwLock::new(list)))
    }
}

impl<T> Clone for ThreadSafeEntry<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[allow(unused)]
impl<T> ThreadSafeEntry<T> {
    #[inline]
    #[must_use]
    pub fn active_slots(&self) -> usize {
        self.0.read().unwrap().active_slots()
    }

    /// Returns total entries currently parked.
    #[inline]
    #[must_use]
    pub fn parked_slots(&self) -> usize {
        self.0.read().unwrap().parked_slots()
    }

    /// `allocated_slots` returns how many slots have being allocated overall.
    #[inline]
    #[must_use]
    pub fn allocated_slots(&self) -> usize {
        self.0.read().unwrap().allocated_slots()
    }

    /// `open_slots` returns how many free entries are now available.
    #[inline]
    #[must_use]
    pub fn open_slots(&self) -> usize {
        self.0.read().unwrap().open_slots()
    }

    /// `get_mut` lets you perform an in-place value replacement without
    /// invalidating the `Entry` handle you have
    /// pointing to the given value it points to.
    ///
    /// This is useful in those cases where all you really wish to do is
    /// update the underlying value without loosing your key like you would
    /// a regular map.
    #[inline]
    pub fn get_mut<Func: FnOnce(Option<&mut T>)>(&self, entry: &Entry, fnc: Func) {
        let mut handle = self.0.write().unwrap();
        fnc(handle.get_mut(entry));
    }

    /// get a reference to the relevant value within the
    /// list for the giving `Entry` if its still valid.
    #[inline]
    pub fn get<Func: FnOnce(Option<&T>)>(&self, entry: &Entry, fnc: Func) {
        let handle = self.0.read().unwrap();
        fnc(handle.get(entry));
    }

    /// `not_valid` returns bool (True/False) indicating if the entry
    /// reference is still valid.
    #[inline]
    #[must_use]
    pub fn not_valid(&self, entry: &Entry) -> bool {
        self.0.write().unwrap().not_valid(entry)
    }

    /// has returns bool (True/False) indicating if the entry
    /// exists and is still valid.
    #[inline]
    #[must_use]
    pub fn has(&self, entry: &Entry) -> bool {
        self.0.write().unwrap().has(entry)
    }

    /// vacate eats the value at that location in the list
    /// freeing the entry for re-use if not already.
    ///
    /// The old value is dropped if it indeed is valid/has-value.
    #[inline]
    pub fn vacate(&self, entry: &Entry) {
        self.0.write().unwrap().vacate(entry);
    }

    /// pack collects the value pointed to by the relevant
    /// `Entry` pointer if its still valid but does not invalidate
    /// the pointer.
    /// You can think of this as a temporary borrow where we want
    /// to borrow that given entry, use it for some undefined period of
    /// time and be guaranteed that slot will not be usable till it's unpacked
    /// basically, you own that entry till its unpacked.
    ///
    /// This allows us support situations where we need to maintain that entry
    /// but cant afford to invalid the entry due to dependency chains built on it.
    #[inline]
    #[must_use]
    pub fn park(&self, entry: &Entry) -> Option<T> {
        self.0.write().unwrap().park(entry)
    }

    /// unpack helps you to re-allocate the provided value
    /// back into the packed entry, if the entry was truly
    /// packaed then true is returned to validate that
    /// the entry was indeed found and updated.
    pub fn unpark(&self, entry: &Entry, item: T) -> bool {
        self.0.write().unwrap().unpark(entry, item)
    }

    #[must_use]
    pub fn find_packed(&self, entry: &Entry) -> Option<usize> {
        self.0.write().unwrap().find_packed(entry)
    }

    #[inline]
    pub fn update_packed(&self, entry: &Entry, item: T) -> Option<T> {
        self.0.write().unwrap().update_packed(entry, item)
    }

    /// take collects the value pointed to by the relevant
    /// `Entry` pointer if its still valid and then invalidates
    /// the pointer.
    #[inline]
    #[must_use]
    pub fn take(&self, entry: &Entry) -> Option<T> {
        self.0.write().unwrap().take(entry)
    }

    /// update lets you change the underlying data for a giving reference
    /// without invalidating the reference for that object
    /// and returning old value.
    #[inline]
    pub fn update(&self, entry: &Entry, item: T) -> Option<T> {
        self.0.write().unwrap().update(entry, item)
    }

    /// `for_each` loop through all active entries.
    #[inline]
    pub fn map_with<V>(&self, tn: impl Fn(&T) -> Option<V>) -> Vec<V> {
        self.0.read().unwrap().map_with(tn)
    }

    /// `for_each` loop through all active entries.
    #[inline]
    pub fn for_each(&self, tn: impl Fn(Option<&T>)) {
        self.0.read().unwrap().for_each(tn);
    }

    /// `select_take` loop through all active entries, using the provided
    /// function as a filter and takes the relevant matching values
    /// out of the list returned as an `Vec<T>`.
    ///
    /// This becomes heavly useful when you wish to take a series of underlying
    /// values that match a condition and vacate the underlying entries to be available
    /// for reuse.
    #[inline]
    pub fn select_take(&self, tn: impl Fn(&T) -> bool) -> Vec<T> {
        self.0.write().unwrap().select_take(tn)
    }

    /// replace lets you change the underlying data for a giving reference
    /// invalidating the previous reference for that object
    /// and returning the new reference and old value.
    #[inline]
    pub fn replace(&self, entry: &Entry, item: T) -> Option<(Entry, T)> {
        self.0.write().unwrap().replace(entry, item)
    }

    /// inserts a new value into the list receiving the relevant
    /// `Entry` handle for the item.
    #[inline]
    pub fn insert(&self, item: T) -> Entry {
        self.0.write().unwrap().insert(item)
    }
}

#[cfg(test)]
mod test_entry_list {
    use tracing_test::traced_test;

    use super::*;

    #[test]
    fn entry_list_insert_reference() {
        let mut list: EntryList<&usize> = EntryList::new();
        let entry = list.insert(&1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&&1), list.get(&entry));
        assert_eq!(Some(&mut &1), list.get_mut(&entry));
    }

    #[test]
    fn entry_list_multi_insert_reference() {
        let mut list: EntryList<&usize> = EntryList::new();
        let entry = list.insert(&1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&&1), list.get(&entry));
        assert_eq!(Some(&mut &1), list.get_mut(&entry));

        let entry2 = list.insert(&2);
        assert_eq!(entry2, Entry::new(1, 0));

        let entry3 = list.insert(&3);
        assert_eq!(entry3, Entry::new(2, 0));
    }

    #[test]
    fn entry_list_can_select_take() {
        let mut list: EntryList<usize> = EntryList::new();
        list.insert(1);
        list.insert(2);

        assert_eq!(2, list.allocated_slots());
        assert_eq!(0, list.open_slots());
        assert_eq!(2, list.active_slots());

        let values = list.select_take(|_| true);
        assert_eq!(vec![1, 2], values);

        assert_eq!(2, list.allocated_slots());
        assert_eq!(2, list.open_slots());
        assert_eq!(0, list.active_slots());
    }

    #[test]
    fn entry_list_can_park_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));
        assert_eq!(Some(&mut 1), list.get_mut(&entry));

        assert_eq!(Some(1), list.park(&entry));

        assert_eq!(1, list.allocated_slots());
        assert_eq!(0, list.open_slots());
        assert_eq!(1, list.parked_slots());
        assert_eq!(1, list.active_slots());

        assert!(list.unpark(&entry, 2));
        assert_eq!(Some(&2), list.get(&entry));

        assert_eq!(1, list.allocated_slots());
        assert_eq!(0, list.open_slots());
        assert_eq!(0, list.parked_slots());
        assert_eq!(1, list.active_slots());
    }

    #[test]
    fn entry_list_can_take_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));
        assert_eq!(Some(&mut 1), list.get_mut(&entry));

        assert_eq!(Some(1), list.take(&entry));
        assert_eq!(1, list.allocated_slots());
        assert_eq!(1, list.open_slots());
        assert_eq!(0, list.active_slots());
    }

    #[test]
    fn entry_list_insert_value() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));
        assert_eq!(Some(&mut 1), list.get_mut(&entry));
    }

    #[test]
    fn entry_list_can_vacate_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));
        list.vacate(&entry);
        assert_eq!(None, list.get(&entry));

        assert_eq!(1, list.allocated_slots());
        assert_eq!(1, list.open_slots());
        assert_eq!(0, list.active_slots());
    }

    #[test]
    fn entry_list_can_check_entry_validity() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));

        assert!(list.has(&entry));
    }

    #[test]
    fn entry_list_can_check_if_is_invalid_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));

        let (_, old_value) = list.replace(&entry, 2).expect("should have value");
        assert_eq!(1, old_value);

        assert!(list.not_valid(&entry));
    }

    #[test]
    #[traced_test]
    fn entry_list_can_replace_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));

        let (new_entry, old_value) = list.replace(&entry, 2).expect("should have value");
        assert_eq!(new_entry, Entry::new(0, 1));
        assert_eq!(1, old_value);

        assert_eq!(None, list.get(&entry));
        assert_eq!(Some(&2), list.get(&new_entry));
        assert_eq!(new_entry.id, entry.id);
    }

    #[test]
    fn entry_list_can_update_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));
        assert_eq!(Some(1), list.update(&entry, 2));
        assert_eq!(Some(&2), list.get(&entry));
    }

    #[test]
    fn entry_list_can_modify_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry::new(0, 0));

        assert_eq!(Some(&1), list.get(&entry));

        if let Some(value) = list.get_mut(&entry) {
            *value = 2;
        }

        assert_eq!(Some(&2), list.get(&entry));
    }
}

#[cfg(test)]
mod test_entry_list_edge_cases {
    use super::*;

    #[test]
    fn test_for_each_iteration() {
        let mut list: EntryList<usize> = EntryList::new();
        list.insert(1);
        list.insert(2);
        list.insert(3);

        let sum = std::sync::Arc::new(std::sync::Mutex::new(0));
        let sum_clone = sum.clone();

        list.for_each(|value| {
            if let Some(&v) = value {
                *sum_clone.lock().unwrap() += v;
            }
        });

        assert_eq!(6, *sum.lock().unwrap());
    }

    #[test]
    fn test_map_with_transformation() {
        let mut list: EntryList<usize> = EntryList::new();
        list.insert(1);
        list.insert(2);
        list.insert(3);

        let doubled: Vec<usize> = list.map_with(|&v| Some(v * 2));

        assert_eq!(vec![2, 4, 6], doubled);
    }

    #[test]
    fn test_map_with_filtering() {
        let mut list: EntryList<usize> = EntryList::new();
        list.insert(1);
        list.insert(2);
        list.insert(3);
        list.insert(4);

        let evens: Vec<usize> = list.map_with(|&v| if v % 2 == 0 { Some(v) } else { None });

        assert_eq!(vec![2, 4], evens);
    }

    #[test]
    fn test_large_scale_insertions() {
        let mut list: EntryList<usize> = EntryList::new();
        let count = 1000;

        let mut entries = Vec::new();
        for i in 0..count {
            entries.push(list.insert(i));
        }

        assert_eq!(count, list.active_slots());
        assert_eq!(count, list.allocated_slots());
        assert_eq!(0, list.open_slots());

        // Verify random access
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(Some(&i), list.get(entry));
        }
    }

    #[test]
    fn test_alternating_insert_remove() {
        let mut list: EntryList<usize> = EntryList::new();

        for i in 0..100 {
            let entry = list.insert(i);
            assert_eq!(1, list.active_slots());

            list.take(&entry);
            assert_eq!(0, list.active_slots());
            assert_eq!(1, list.open_slots());
        }

        // After 100 cycles, we should have reused slots
        assert_eq!(1, list.allocated_slots());
        assert_eq!(1, list.open_slots());
    }

    #[test]
    fn test_generation_increment_on_reuse() {
        let mut list: EntryList<usize> = EntryList::new();

        let entry1 = list.insert(1);
        list.take(&entry1);

        let entry2 = list.insert(2);

        // entry2 should have same id but different generation
        assert!(entry1.id == entry2.id);
        assert!(entry1.gen != entry2.gen);

        // Old entry should be invalid
        assert!(list.not_valid(&entry1));
        assert!(list.has(&entry2));
    }

    #[test]
    fn test_multiple_select_take_operations() {
        let mut list: EntryList<usize> = EntryList::new();

        for i in 0..10 {
            list.insert(i);
        }

        // Take all even numbers
        let evens = list.select_take(|&v| v % 2 == 0);
        assert_eq!(5, evens.len());
        assert_eq!(5, list.active_slots());

        // Take all remaining numbers > 5
        let high = list.select_take(|&v| v > 5);
        assert_eq!(2, high.len());
        assert_eq!(3, list.active_slots());
    }

    #[test]
    fn test_vacate_vs_take() {
        let mut list: EntryList<String> = EntryList::new();

        let entry1 = list.insert("value1".to_string());
        let entry2 = list.insert("value2".to_string());

        // vacate drops without returning
        list.vacate(&entry1);
        assert_eq!(None, list.get(&entry1));
        assert_eq!(1, list.open_slots());

        // take returns the value
        let value = list.take(&entry2);
        assert_eq!(Some("value2".to_string()), value);
        assert_eq!(2, list.open_slots());
    }

    #[test]
    fn test_park_and_unpark_cycle() {
        let mut list: EntryList<usize> = EntryList::new();

        let entry = list.insert(100);
        assert_eq!(1, list.active_slots());
        assert_eq!(0, list.parked_slots());

        // Park the entry
        let parked_value = list.park(&entry);
        assert_eq!(Some(100), parked_value);
        assert_eq!(1, list.active_slots()); // Still counts as active
        assert_eq!(1, list.parked_slots());
        assert_eq!(None, list.get(&entry)); // But value is None

        // Unpark with different value
        assert!(list.unpark(&entry, 200));
        assert_eq!(Some(&200), list.get(&entry));
        assert_eq!(0, list.parked_slots());
        assert_eq!(1, list.active_slots());
    }

    #[test]
    fn test_unpark_nonexistent_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = Entry::new(999, 0);

        // Trying to unpark a non-existent entry should fail
        assert!(!list.unpark(&entry, 100));
    }

    #[test]
    fn test_replace_invalidates_old_entry() {
        let mut list: EntryList<usize> = EntryList::new();

        let entry1 = list.insert(10);
        assert!(list.has(&entry1));

        let (entry2, old_value) = list.replace(&entry1, 20).unwrap();
        assert_eq!(10, old_value);

        // entry1 is now invalid
        assert!(list.not_valid(&entry1));
        assert!(!list.has(&entry1));

        // entry2 is valid with new value
        assert!(list.has(&entry2));
        assert_eq!(Some(&20), list.get(&entry2));
    }

    #[test]
    fn test_mixed_operations_stress() {
        let mut list: EntryList<usize> = EntryList::new();
        let mut active_entries = Vec::new();

        // Insert initial batch
        for i in 0..50 {
            active_entries.push(list.insert(i));
        }

        // Remove half
        for i in (0..25).rev() {
            list.take(&active_entries.remove(i));
        }

        assert_eq!(25, list.active_slots());
        assert_eq!(25, list.open_slots());

        // Update some
        for entry in active_entries.iter().take(10) {
            list.update(entry, 999);
        }

        // Replace some (invalidates old entries)
        let mut new_entries = Vec::new();
        for entry in active_entries.iter().skip(10).take(5) {
            if let Some((new_entry, _)) = list.replace(entry, 888) {
                new_entries.push(new_entry);
            }
        }

        // Verify integrity
        for entry in new_entries.iter() {
            assert_eq!(Some(&888), list.get(entry));
        }
    }
}

#[cfg(test)]
mod test_thread_safe_entry {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_thread_safe_entry_new() {
        let safe_list: ThreadSafeEntry<usize> = ThreadSafeEntry::new();
        assert_eq!(0, safe_list.active_slots());
    }

    #[test]
    fn test_thread_safe_entry_default() {
        let safe_list: ThreadSafeEntry<usize> = ThreadSafeEntry::default();
        assert_eq!(0, safe_list.active_slots());
    }

    #[test]
    fn test_thread_safe_entry_from_list() {
        let mut list: EntryList<usize> = EntryList::new();
        list.insert(1);
        list.insert(2);

        let safe_list = ThreadSafeEntry::from(list);
        assert_eq!(2, safe_list.active_slots());
    }

    #[test]
    fn test_thread_safe_entry_concurrent_inserts() {
        let safe_list = Arc::new(ThreadSafeEntry::<usize>::new());
        let mut handles = vec![];

        for i in 0..10 {
            let list_clone = safe_list.clone();
            handles.push(thread::spawn(move || list_clone.insert(i)));
        }

        let entries: Vec<Entry> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(10, safe_list.active_slots());
        assert_eq!(10, entries.len());
    }

    #[test]
    fn test_thread_safe_entry_concurrent_reads() {
        let safe_list = Arc::new(ThreadSafeEntry::<usize>::new());

        let entries: Vec<Entry> = (0..5).map(|i| safe_list.insert(i * 10)).collect();

        let mut handles = vec![];
        for entry in entries {
            let list_clone = safe_list.clone();
            handles.push(thread::spawn(move || {
                let mut result = None;
                list_clone.get(&entry, |value| {
                    result = value.copied();
                });
                result
            }));
        }

        let results: Vec<Option<usize>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(5, results.iter().filter(|r| r.is_some()).count());
    }

    #[test]
    fn test_thread_safe_entry_get_mut() {
        let safe_list = ThreadSafeEntry::<usize>::new();
        let entry = safe_list.insert(10);

        safe_list.get_mut(&entry, |value| {
            if let Some(v) = value {
                *v = 20;
            }
        });

        let mut result = None;
        safe_list.get(&entry, |value| {
            result = value.copied();
        });

        assert_eq!(Some(20), result);
    }

    #[test]
    fn test_thread_safe_entry_park_unpark() {
        let safe_list = ThreadSafeEntry::<usize>::new();
        let entry = safe_list.insert(100);

        assert_eq!(0, safe_list.parked_slots());

        let parked = safe_list.park(&entry);
        assert_eq!(Some(100), parked);
        assert_eq!(1, safe_list.parked_slots());

        assert!(safe_list.unpark(&entry, 200));
        assert_eq!(0, safe_list.parked_slots());

        let mut result = None;
        safe_list.get(&entry, |value| {
            result = value.copied();
        });
        assert_eq!(Some(200), result);
    }

    #[test]
    fn test_thread_safe_entry_concurrent_mixed_ops() {
        let safe_list = Arc::new(ThreadSafeEntry::<usize>::new());

        // Insert initial entries
        let entries: Vec<Entry> = (0..20).map(|i| safe_list.insert(i)).collect();

        let mut read_handles = vec![];
        let mut update_handles = vec![];

        // Spawn readers
        for entry in entries.iter().take(10) {
            let list_clone = safe_list.clone();
            let entry_copy = *entry;
            read_handles.push(thread::spawn(move || {
                let mut found = false;
                list_clone.get(&entry_copy, |value| {
                    found = value.is_some();
                });
                found
            }));
        }

        // Spawn updaters
        for entry in entries.iter().skip(10) {
            let list_clone = safe_list.clone();
            let entry_copy = *entry;
            update_handles.push(thread::spawn(move || list_clone.update(&entry_copy, 999)));
        }

        // Wait for all operations
        for handle in read_handles {
            handle.join().unwrap();
        }
        for handle in update_handles {
            handle.join().unwrap();
        }

        assert_eq!(20, safe_list.active_slots());
    }

    #[test]
    fn test_thread_safe_entry_select_take() {
        let safe_list = ThreadSafeEntry::<usize>::new();

        for i in 0..10 {
            safe_list.insert(i);
        }

        let evens = safe_list.select_take(|&v| v % 2 == 0);
        assert_eq!(5, evens.len());
        assert_eq!(5, safe_list.active_slots());
    }

    #[test]
    fn test_thread_safe_entry_map_with() {
        let safe_list = ThreadSafeEntry::<usize>::new();

        for i in 1..=5 {
            safe_list.insert(i);
        }

        let doubled: Vec<usize> = safe_list.map_with(|&v| Some(v * 2));
        assert_eq!(vec![2, 4, 6, 8, 10], doubled);
    }

    #[test]
    fn test_thread_safe_entry_for_each() {
        let safe_list = ThreadSafeEntry::<usize>::new();

        for i in 1..=5 {
            safe_list.insert(i);
        }

        let sum = std::sync::Arc::new(std::sync::Mutex::new(0));
        let sum_clone = sum.clone();

        safe_list.for_each(|value| {
            if let Some(&v) = value {
                *sum_clone.lock().unwrap() += v;
            }
        });

        assert_eq!(15, *sum.lock().unwrap());
    }
}
