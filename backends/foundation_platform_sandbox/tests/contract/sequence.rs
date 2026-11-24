//! I6: the sequence accepts only the next number.

use std::cmp::Ordering;

use foundation_platform_sandbox::shared::sequence::{Sequence, SequenceCheck};
use proptest::prelude::*;

use super::common;

/// The one property configuration of this file.
fn config() -> ProptestConfig {
    common::config(0x5eed_0003)
}

proptest! {
    #![proptest_config(config())]

    /// I6 against a model that is a plain counter.
    #[test]
    fn sequence_accept(numbers in proptest::collection::vec(0u64..40, 0..48)) {
        let mut sequence = Sequence::new();
        let mut model = 0u64;
        for number in numbers {
            let expected = match number.cmp(&model) {
                Ordering::Less => SequenceCheck::Duplicate,
                Ordering::Greater => SequenceCheck::Gap,
                Ordering::Equal => {
                    model += 1;
                    SequenceCheck::Apply
                }
            };
            prop_assert_eq!(sequence.accept(number), expected);
            prop_assert_eq!(sequence.next(), model);
        }
    }
}

#[test]
fn sequence_accept_scenarios() {
    let mut sequence = Sequence::new();
    assert_eq!(sequence.accept(0), SequenceCheck::Apply);
    assert_eq!(sequence.accept(1), SequenceCheck::Apply);
    assert_eq!(sequence.next(), 2);

    let mut sequence = Sequence::new();
    assert_eq!(sequence.accept(0), SequenceCheck::Apply);
    assert_eq!(sequence.accept(1), SequenceCheck::Apply);
    assert_eq!(sequence.accept(1), SequenceCheck::Duplicate);
    assert_eq!(sequence.next(), 2);

    assert_eq!(sequence.accept(3), SequenceCheck::Gap);
    assert_eq!(sequence.next(), 2);
}

proptest! {
    #![proptest_config(config())]

    /// A span advances by its length, and an overlapping span is a gap.
    #[test]
    fn sequence_span(spans in proptest::collection::vec((0u64..40, 0u64..8), 0..32)) {
        let mut sequence = Sequence::new();
        let mut model = 0u64;
        for (offset, len) in spans {
            let expected = if offset == model {
                model += len;
                SequenceCheck::Apply
            } else if offset + len <= model {
                SequenceCheck::Duplicate
            } else {
                SequenceCheck::Gap
            };
            prop_assert_eq!(sequence.accept_span(offset, len), expected);
            prop_assert_eq!(sequence.next(), model);
        }
    }
}

#[test]
fn sequence_span_scenarios() {
    let mut sequence = Sequence::new();
    assert_eq!(sequence.accept_span(0, 5), SequenceCheck::Apply);
    assert_eq!(sequence.next(), 5);

    assert_eq!(sequence.accept_span(0, 5), SequenceCheck::Duplicate);
    assert_eq!(sequence.next(), 5);

    // A span inside the accepted range is a duplicate.
    assert_eq!(sequence.accept_span(2, 2), SequenceCheck::Duplicate);
    assert_eq!(sequence.next(), 5);

    // A span that overlaps the end is a gap.
    assert_eq!(sequence.accept_span(2, 10), SequenceCheck::Gap);
    assert_eq!(sequence.next(), 5);

    // The next span applies.
    assert_eq!(sequence.accept_span(5, 3), SequenceCheck::Apply);
    assert_eq!(sequence.next(), 8);
}

/// A counter at `u64::MAX` cannot advance, and nothing overflows.
#[test]
fn sequence_at_u64_max() {
    let mut sequence = Sequence::new();
    assert_eq!(sequence.accept_span(0, u64::MAX), SequenceCheck::Apply);
    assert_eq!(sequence.next(), u64::MAX);

    assert_eq!(sequence.accept(u64::MAX), SequenceCheck::Gap);
    assert_eq!(sequence.next(), u64::MAX);

    assert_eq!(sequence.accept_span(u64::MAX, 1), SequenceCheck::Gap);
    assert_eq!(sequence.next(), u64::MAX);
}
