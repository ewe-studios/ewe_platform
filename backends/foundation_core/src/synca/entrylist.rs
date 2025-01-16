/// Entry based list using generation markers to identify
/// used list items in an efficient list.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Entry {
    id: usize,
    gen: usize,
}

#[allow(dead_code)]
impl Entry {
    pub(crate) fn new(id: usize, gen: usize) -> Self {
        Self { id, gen }
    }
}

#[derive(Debug, Clone)]
pub struct EntryList<T> {
    items: Vec<(usize, Option<T>)>,
    free_entrys: Vec<Entry>,
}

// --- constructors

impl<T> EntryList<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            free_entrys: Vec::new(),
        }
    }
}

// --- methods

impl<T> EntryList<T> {
    /// active_slots returns how many slots have actually still have value.
    /// Basically does a match of
    /// `EntryList::allocated_slots()` - `EntryList::active_slots()`.
    pub fn active_slots(&self) -> usize {
        self.items.len() - self.free_entrys.len()
    }

    /// open_slots returns how many slots have being allocated overall.
    pub fn allocated_slots(&self) -> usize {
        self.items.len()
    }

    /// open_slots returns how many free entries are now available.
    pub fn open_slots(&self) -> usize {
        self.free_entrys.len()
    }

    /// get_mut lets you perform an in-place value replacement without
    /// invalidating the `Entry` handle you have
    /// pointing to the given value it points to.
    ///
    /// This is useful in those cases where all you really wish to do is
    /// update the underlying value without loosing your key like you would
    /// a regular map.
    pub fn get_mut(&mut self, entry: &Entry) -> Option<&mut T> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                if let Some(_) = &value {
                    return value.as_mut();
                }
            }
        }
        None
    }

    /// get a reference to the relevant value within the
    /// list for the giving `Entry` if its still valid.
    pub fn get(&self, entry: &Entry) -> Option<&T> {
        if let Some((gen, value)) = self.items.get(entry.id) {
            if *gen == entry.gen {
                if value.is_some() {
                    return value.as_ref();
                }
            }
        }
        None
    }

    /// not_valid returns bool (True/False) indicating if the entry
    /// reference is still valid.
    pub fn not_valid(&mut self, entry: &Entry) -> bool {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen != entry.gen {
                return true;
            }

            if *gen == entry.gen {
                if value.is_none() {
                    return true;
                }
            }
        }
        false
    }

    /// has returns bool (True/False) indicating if the entry
    /// exists and is still valid.
    pub fn has(&mut self, entry: &Entry) -> bool {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                if value.is_some() {
                    return true;
                }
            }
        }
        false
    }

    /// vacate eats the value at that location in the list
    /// freeing the entry for re-use if not already.
    ///
    /// The old value is dropped if it indeed is valid/has-value.
    pub fn vacate(&mut self, entry: &Entry) {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                if let Some(con) = value.take() {
                    self.free_entrys.push(entry.clone());
                    drop(con);
                }
            }
        }
    }

    /// take collects the value pointed to by the relevant
    /// `Entry` pointer if its still valid and then invalidates
    /// the pointer.
    pub fn take(&mut self, entry: &Entry) -> Option<T> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                if let Some(con) = value.take() {
                    self.free_entrys.push(entry.clone());
                    return Some(con);
                }
            }
        }
        None
    }

    /// update lets you change the underlying data for a giving reference
    /// without invalidating the reference for that object
    /// and returning old value.
    pub fn update(&mut self, entry: &Entry, item: T) -> Option<T> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                if value.is_some() {
                    // collect old value
                    let previous_value = value.take();

                    // replace value
                    *value = Some(item);

                    // Return new Entry and Old value.
                    return previous_value;
                }
            }
        }
        None
    }

    /// for_each loop through all active entries.
    pub fn map_with<V>(&self, tn: impl Fn(&T) -> Option<V>) -> Vec<V>  {
        self.items.iter().map(|(_gen, value)| -> Option<V> {
            if value.is_none() {
                return None;
            }

            match value {
                Some(item) => tn(item),
                None => None
            }
        }).filter(|item| item.is_some()).map(|item| item.unwrap()).collect()
    }

    /// for_each loop through all active entries.
    pub fn for_each(&self, tn: impl Fn(Option<&T>)) {
        self.items.iter().for_each(|(_gen, value)| {
            if value.is_none() {
                return;
            }

            tn(value.as_ref())
        });
    }

    /// select_take loop through all active entries, using the provided
    /// function as a filter and takes the relevant matching values
    /// out of the list returned as an `Vec<T>`.
    ///
    /// This becomes heavly useful when you wish to take a series of underlying
    /// values that match a condition and vacate the underlying entries to be available
    /// for reuse.
    pub fn select_take(&mut self, tn: impl Fn(&T) -> bool) -> Vec<T> {
        self.items
            .iter_mut()
            .enumerate()
            .filter(|(_index, (_gen, value))| match value {
                Some(inner) => tn(inner),
                None => false,
            })
            .map(|(index, (gen, value))| {
                self.free_entrys.push(Entry::new(index, *gen));
                value.take().unwrap()
            })
            .collect()
    }

    /// replace lets you change the underlying data for a giving reference
    /// invalidating the previous reference for that object
    /// and returning the new reference and old value.
    pub fn replace(&mut self, entry: &Entry, item: T) -> Option<(Entry, T)> {
        if let Some((gen, value)) = self.items.get_mut(entry.id) {
            if *gen == entry.gen {
                let new_gen = *gen + 1;
                if value.is_some() {
                    // collect old value
                    let previous_value = value.take();

                    // replace gen
                    *gen = new_gen;

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
    pub fn insert(&mut self, item: T) -> Entry {
        let entry = match self.free_entrys.pop() {
            Some(mut inner) => {
                inner.gen += 1;
                inner
            }
            None => {
                let inner = Entry {
                    id: self.items.len(),
                    gen: 0,
                };

                inner
            }
        };

        if self.items.len() == entry.id {
            self.items.push((entry.gen, Some(item)));
        } else {
            self.items[entry.id] = (entry.gen, Some(item));
        }

        entry
    }
}

#[cfg(test)]
mod test_entry_list {
    use super::*;

    #[test]
    fn entry_list_insert_reference() {
        let mut list: EntryList<&usize> = EntryList::new();
        let entry = list.insert(&1);
        assert_eq!(entry, Entry { id: 0, gen: 0 });

        assert_eq!(Some(&&1), list.get(&entry));
        assert_eq!(Some(&mut &1), list.get_mut(&entry));
    }

    #[test]
    fn entry_list_multi_insert_reference() {
        let mut list: EntryList<&usize> = EntryList::new();
        let entry = list.insert(&1);
        assert_eq!(entry, Entry { id: 0, gen: 0 });

        assert_eq!(Some(&&1), list.get(&entry));
        assert_eq!(Some(&mut &1), list.get_mut(&entry));

        let entry2 = list.insert(&2);
        assert_eq!(entry2, Entry { id: 1, gen: 0 });

        let entry3 = list.insert(&3);
        assert_eq!(entry3, Entry { id: 2, gen: 0 });
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
    fn entry_list_can_take_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry { id: 0, gen: 0 });

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
        assert_eq!(entry, Entry { id: 0, gen: 0 });

        assert_eq!(Some(&1), list.get(&entry));
        assert_eq!(Some(&mut 1), list.get_mut(&entry));
    }

    #[test]
    fn entry_list_can_vacate_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry { id: 0, gen: 0 });

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
        assert_eq!(entry, Entry { id: 0, gen: 0 });

        assert_eq!(Some(&1), list.get(&entry));

        assert!(list.has(&entry));
    }

    #[test]
    fn entry_list_can_check_if_is_invalid_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry { id: 0, gen: 0 });

        assert_eq!(Some(&1), list.get(&entry));

        let (_, old_value) = list.replace(&entry, 2).expect("should have value");
        assert_eq!(1, old_value);

        assert!(list.not_valid(&entry));
    }

    #[test]
    fn entry_list_can_replace_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry { id: 0, gen: 0 });

        assert_eq!(Some(&1), list.get(&entry));

        let (new_entry, old_value) = list.replace(&entry, 2).expect("should have value");
        assert_eq!(new_entry, Entry { id: 0, gen: 1 });
        assert_eq!(1, old_value);

        assert_eq!(None, list.get(&entry));
        assert_eq!(Some(&2), list.get(&new_entry));
        assert_eq!(new_entry.id, entry.id);
    }

    #[test]
    fn entry_list_can_update_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry { id: 0, gen: 0 });

        assert_eq!(Some(&1), list.get(&entry));
        assert_eq!(Some(1), list.update(&entry, 2));
        assert_eq!(Some(&2), list.get(&entry));
    }

    #[test]
    fn entry_list_can_modify_entry() {
        let mut list: EntryList<usize> = EntryList::new();
        let entry = list.insert(1);
        assert_eq!(entry, Entry { id: 0, gen: 0 });

        assert_eq!(Some(&1), list.get(&entry));

        if let Some(value) = list.get_mut(&entry) {
            *value = 2;
        }

        assert_eq!(Some(&2), list.get(&entry));
    }
}
