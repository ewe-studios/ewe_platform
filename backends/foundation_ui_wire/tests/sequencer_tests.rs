use foundation_ui_wire::writer::{FrameHeader, StreamSequencer};

#[test]
fn next_is_strictly_increasing() {
    let mut sequencer = StreamSequencer::new(7);
    assert_eq!(sequencer.stream_id(), 7);
    assert_eq!(sequencer.last(), 0);
    for next in 1..=1000 {
        assert_eq!(sequencer.next(), next);
        assert_eq!(sequencer.last(), next);
    }
}

#[test]
fn resume_continues_after_last() {
    let mut sequencer = StreamSequencer::resume(7, 20);
    assert_eq!(sequencer.last(), 20);
    assert_eq!(
        sequencer.header(5),
        FrameHeader {
            stream_id: 7,
            sequence_id: 21,
            ack: 5
        }
    );
    assert_eq!(sequencer.next(), 22);
}

#[test]
fn sequence_saturates_at_maximum() {
    let mut sequencer = StreamSequencer::resume(7, u64::MAX - 1);
    assert_eq!(sequencer.next(), u64::MAX);
    assert_eq!(sequencer.next(), u64::MAX);
    assert_eq!(sequencer.last(), u64::MAX);
}
