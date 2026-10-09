use foundation_core::io::ubytes::BytesPointer;

#[test]
fn test_can_use_accumulator_to_peek_next_character() {
    let mut accumulator = BytesPointer::new(b"hello");
    assert_eq!(b"h", accumulator.peek(1).unwrap());
    assert_eq!(b"h", accumulator.peek(1).unwrap());
}

#[test]
fn test_can_use_accumulator_to_peek_two_characters_away() {
    let mut accumulator = BytesPointer::new(b"hello");
    assert_eq!(b"he", accumulator.peek(2).unwrap());
}

#[test]
fn test_can_virtual_peek_ahead_without_changing_peek_cursor() {
    let mut accumulator = BytesPointer::new(b"hello");

    assert_eq!(b"h", accumulator.peek_next().unwrap());
    assert_eq!(b"e", accumulator.peek_next().unwrap());

    assert_eq!(b"llo", accumulator.vpeek_at(0, 3).unwrap()); // from peek cursor till 3 ahead
    assert_eq!(b"lo", accumulator.vpeek_at(1, 3).unwrap()); // from 1 character ahead of peek cursor

    assert_eq!(b"l", accumulator.peek_next().unwrap());
    assert_eq!(b"l", accumulator.peek_next().unwrap());
    assert_eq!(b"o", accumulator.peek_next().unwrap());
    assert_eq!(None, accumulator.peek_next());
}

#[test]
fn test_can_peek_next_to_accumulate_more_seen_text() {
    let mut accumulator = BytesPointer::new(b"hello");

    assert_eq!(b"h", accumulator.peek_next().unwrap());
    assert_eq!(b"e", accumulator.peek_next().unwrap());
    assert_eq!(b"l", accumulator.peek_next().unwrap());
    assert_eq!(b"l", accumulator.peek_next().unwrap());
    assert_eq!(b"o", accumulator.peek_next().unwrap());

    assert_eq!(None, accumulator.peek_next());
}

#[test]
fn test_can_peek_next_and_take_text_then_continue_peeking() {
    let mut accumulator = BytesPointer::new(b"hello");

    assert_eq!(5, accumulator.len());

    assert_eq!(b"h", accumulator.peek_next().unwrap());
    assert_eq!(b"e", accumulator.peek_next().unwrap());
    assert_eq!(b"l", accumulator.peek_next().unwrap());

    assert_eq!(5, accumulator.len());
    assert_eq!(5, accumulator.rem_len());
    assert_eq!(2, accumulator.peek_rem_len());

    assert_eq!(b"hel", accumulator.take().unwrap());

    assert_eq!(2, accumulator.rem_len());
    assert_eq!(2, accumulator.peek_rem_len());

    assert_eq!(b"l", accumulator.peek_next().unwrap());
    assert_eq!(b"o", accumulator.peek_next().unwrap());

    assert_eq!(2, accumulator.rem_len());
    assert_eq!(0, accumulator.peek_rem_len());

    assert_eq!(None, accumulator.peek_next());

    assert_eq!(2, accumulator.rem_len());
    assert_eq!(0, accumulator.peek_rem_len());

    assert_eq!(b"lo", accumulator.take().unwrap());

    assert_eq!(0, accumulator.rem_len());

    assert_eq!(None, accumulator.peek_next());
}
