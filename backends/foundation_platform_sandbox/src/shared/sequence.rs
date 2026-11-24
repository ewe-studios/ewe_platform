//! A counter that accepts only the next number.

/// The result of a sequence check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceCheck {
    /// The number is the next one, and the counter advanced.
    Apply,
    /// The number is older than the counter. Nothing changed.
    Duplicate,
    /// The number is not the next one and is not older. Nothing changed.
    Gap,
}

/// A counter that accepts only the next number.
///
/// `accept(n)` gives [`SequenceCheck::Apply`] when `n` equals the next number,
/// and adds one. It gives [`SequenceCheck::Duplicate`] for an older number and
/// [`SequenceCheck::Gap`] for a newer one. Neither changes the counter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sequence {
    next: u64,
}

impl Sequence {
    /// A counter at zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next number that the counter accepts.
    #[must_use]
    pub fn next(&self) -> u64 {
        self.next
    }

    /// Accepts the next number of a counter that grows by one.
    ///
    /// A counter at [`u64::MAX`] cannot advance. It reports
    /// [`SequenceCheck::Gap`] and changes nothing.
    pub fn accept(&mut self, number: u64) -> SequenceCheck {
        if number < self.next {
            return SequenceCheck::Duplicate;
        }
        if number > self.next {
            return SequenceCheck::Gap;
        }
        match self.next.checked_add(1) {
            Some(next) => {
                self.next = next;
                SequenceCheck::Apply
            }
            None => SequenceCheck::Gap,
        }
    }

    /// Accepts the next span of a byte offset.
    ///
    /// When `offset` equals the next offset, the counter adds `len` and reports
    /// [`SequenceCheck::Apply`]. A span that ends at or before the counter is a
    /// [`SequenceCheck::Duplicate`]. Every other case is a
    /// [`SequenceCheck::Gap`], including a span that overlaps the end.
    pub fn accept_span(&mut self, offset: u64, len: u64) -> SequenceCheck {
        if offset == self.next {
            return match self.next.checked_add(len) {
                Some(next) => {
                    self.next = next;
                    SequenceCheck::Apply
                }
                None => SequenceCheck::Gap,
            };
        }
        if offset.checked_add(len).is_some_and(|end| end <= self.next) {
            return SequenceCheck::Duplicate;
        }
        SequenceCheck::Gap
    }
}
