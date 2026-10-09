use foundation_core::io::mem::stringpointer::StringPointer;

#[test]
fn test_can_use_accumulator_to_peek_next_character() {
    let mut accumulator = StringPointer::new("hello");
    assert_eq!("h", accumulator.peek(1).unwrap());
    assert_eq!("h", accumulator.peek(1).unwrap());
}

#[test]
fn test_can_use_accumulator_to_peek_two_characters_away() {
    let mut accumulator = StringPointer::new("hello");
    assert_eq!("he", accumulator.peek(2).unwrap());
}

#[test]
fn test_can_virtual_peek_ahead_without_changing_peek_cursor() {
    let mut accumulator = StringPointer::new("hello");

    assert_eq!("h", accumulator.peek_next().unwrap());
    assert_eq!("e", accumulator.peek_next().unwrap());

    assert_eq!("llo", accumulator.vpeek_at(0, 3).unwrap()); // from peek cursor till 3 ahead
    assert_eq!("lo", accumulator.vpeek_at(1, 3).unwrap()); // from 1 character ahead of peek cursor

    assert_eq!("l", accumulator.peek_next().unwrap());
    assert_eq!("l", accumulator.peek_next().unwrap());
    assert_eq!("o", accumulator.peek_next().unwrap());
    assert_eq!(None, accumulator.peek_next());
}

#[test]
fn test_can_peek_next_to_accumulate_more_seen_text() {
    let mut accumulator = StringPointer::new("hello");

    assert_eq!("h", accumulator.peek_next().unwrap());
    assert_eq!("e", accumulator.peek_next().unwrap());
    assert_eq!("l", accumulator.peek_next().unwrap());
    assert_eq!("l", accumulator.peek_next().unwrap());
    assert_eq!("o", accumulator.peek_next().unwrap());

    assert_eq!(None, accumulator.peek_next());
}

#[test]
fn test_can_peek_next_and_take_text_then_continue_peeking() {
    let mut accumulator = StringPointer::new("hello");

    assert_eq!(5, accumulator.len());

    assert_eq!("h", accumulator.peek_next().unwrap());
    assert_eq!("e", accumulator.peek_next().unwrap());
    assert_eq!("l", accumulator.peek_next().unwrap());

    assert_eq!(5, accumulator.len());
    assert_eq!(5, accumulator.rem_len());
    assert_eq!(2, accumulator.peek_rem_len());

    assert_eq!("hel", accumulator.take().unwrap());

    assert_eq!(2, accumulator.rem_len());
    assert_eq!(2, accumulator.peek_rem_len());

    assert_eq!("l", accumulator.peek_next().unwrap());
    assert_eq!("o", accumulator.peek_next().unwrap());

    assert_eq!(2, accumulator.rem_len());
    assert_eq!(0, accumulator.peek_rem_len());

    assert_eq!(None, accumulator.peek_next());

    assert_eq!(2, accumulator.rem_len());
    assert_eq!(0, accumulator.peek_rem_len());

    assert_eq!("lo", accumulator.take().unwrap());

    assert_eq!(0, accumulator.rem_len());

    assert_eq!(None, accumulator.peek_next());
}
