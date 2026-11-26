use foundation_ui_wire::{
    AsyncFrameReader, FrameReader, FramingError, OwnedFrame, StreamFrameWriter, WireLimits,
};
use futures_util::stream;
use std::io::{self, Read, Write};

const FRAME: &[u8] = include_bytes!("../fixtures/frame_ops_counter.bin");

struct OneByte<'a>(&'a [u8]);
impl Read for OneByte<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = out.len().min(1);
        self.0.read(&mut out[..n])
    }
}
struct Short(Vec<u8>);
impl Write for Short {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = b.len().min(2);
        self.0.extend_from_slice(&b[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn fragmented_frames_round_trip_in_order() {
    let limits = WireLimits::default();
    let frame = OwnedFrame::new(FRAME.to_vec(), &limits).unwrap();
    let mut out = Short(vec![]);
    let mut writer = StreamFrameWriter::length_prefixed(&mut out);
    writer.write_frame(&frame).unwrap();
    writer.write_frame(&frame).unwrap();
    let mut reader = FrameReader::length_prefixed(OneByte(&out.0), limits);
    for _ in 0..2 {
        assert_eq!(reader.next().unwrap().unwrap().as_bytes(), FRAME);
    }
    assert!(reader.next().is_none());
    assert_eq!(
        FrameReader::raw(FRAME, limits)
            .next()
            .unwrap()
            .unwrap()
            .view()
            .sequence_id(),
        frame.view().sequence_id()
    );
}

#[test]
fn errors_identify_the_boundary_and_stop() {
    let limits = WireLimits::default().max_frame_bytes(20);
    for (bytes, expected) in [
        (&[4, 0][..], "prefix"),
        (&[4, 0, 0, 0, 1][..], "payload"),
        (&[21, 0, 0, 0][..], "limit"),
        (&[1, 0, 0, 0, 1][..], "wire"),
    ] {
        let mut read = FrameReader::length_prefixed(bytes, limits);
        let actual = match read.next().unwrap().unwrap_err() {
            FramingError::TruncatedPrefix { .. } => "prefix",
            FramingError::TruncatedPayload { .. } => "payload",
            FramingError::Limit { .. } => "limit",
            FramingError::Wire(_) => "wire",
            FramingError::Source(_) => "source",
        };
        assert_eq!(actual, expected);
        assert!(read.next().is_none());
    }
}

#[test]
fn asynchronous_reader_delivers_before_source_end_and_preserves_surplus() {
    use futures_util::StreamExt;
    let mut both = Vec::new();
    let frame = OwnedFrame::new(FRAME.to_vec(), &WireLimits::default()).unwrap();
    let mut writer = StreamFrameWriter::length_prefixed(&mut both);
    writer.write_frame(&frame).unwrap();
    writer.write_frame(&frame).unwrap();
    let input = stream::iter([
        Ok::<_, io::Error>(both[..2].to_vec()),
        Ok(both[2..].to_vec()),
    ])
    .chain(stream::pending());
    let mut reader = AsyncFrameReader::length_prefixed(input, WireLimits::default());
    for _ in 0..2 {
        assert_eq!(
            futures_lite::future::block_on(reader.next_frame())
                .unwrap()
                .unwrap()
                .as_bytes(),
            FRAME
        );
    }
}
#[test]
fn adapters_propagate_source_and_sink_errors() {
    struct Fail;
    impl Read for Fail {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    impl Write for Fail {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let limits = WireLimits::default();
    assert!(matches!(
        FrameReader::raw(Fail, limits).next(),
        Some(Err(FramingError::Source(_)))
    ));
    let mut reader = AsyncFrameReader::raw(
        stream::iter([Err::<Vec<u8>, _>(io::Error::from(
            io::ErrorKind::BrokenPipe,
        ))]),
        limits,
    );
    assert!(matches!(
        futures_lite::future::block_on(reader.next_frame()),
        Err(FramingError::Source(_))
    ));
    let frame = OwnedFrame::new(FRAME.to_vec(), &limits).unwrap();
    let mut writer = StreamFrameWriter::raw(Fail);
    assert_eq!(
        writer.write_frame(&frame).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        writer.write_frame(&frame).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}
