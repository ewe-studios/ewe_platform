#![cfg(test)]

use foundation_core::wire::simple_http::*;
use tracing_test::traced_test;

struct ChunkSample {
    content: &'static [&'static str],
    expected: Vec<ChunkState>,
}

struct TrailerSample {
    content: &'static [&'static str],
    expected: Vec<Option<ChunkState>>,
}

struct LineFeedSample {
    content: &'static [&'static str],
    expected: Vec<LineFeed>,
}

#[test]
#[traced_test]
fn test_chunk_state_parse_http_trailers() {
    let test_cases: Vec<LineFeedSample> = vec![LineFeedSample {
        expected: vec![
            LineFeed::Line("Farm: FarmValue".into()),
            LineFeed::Line("Farm:FarmValue".into()),
            LineFeed::Line("Farm:FarmValue".into()),
            LineFeed::Line("Farm:\rFarmValue".into()),
            LineFeed::Line("Farm:\nFarmValue".into()),
            LineFeed::END,
            // Will not capture joined lines but should in fact parse them one by one
            // Some(LineFeed::Line("Farm: FarmValue\r\nFarm: FarmValue".into())),
            //
            // So instead, we will see:
            LineFeed::Line("Farm: FarmValue".into()),
        ],
        content: &[
            "Farm: FarmValue\r\n",
            "Farm:FarmValue\n\n",
            "Farm:FarmValue\n\n\n",
            "Farm:\rFarmValue\r\n\r\n",
            "Farm:\nFarmValue\r\n\r\n",
            "\r\n",
            // only one portion is extracted here,
            // caller should call the stream again to
            // pull the next chunk within the stream
            "Farm: FarmValue\r\nFarm: FarmValue\r\n",
        ][..],
    }];

    for sample in test_cases {
        let chunks: Result<Vec<LineFeed>, LineFeedError> = sample
            .content
            .iter()
            .map(|t| LineFeed::stream_line_feeds_from_string(t.as_bytes()))
            .collect();

        assert!(chunks.is_ok());
        assert_eq!(chunks.unwrap(), sample.expected);
    }
}

#[test]
fn test_chunk_state_parse_http_trailers_less() {
    let test_cases: Vec<TrailerSample> = vec![TrailerSample {
        expected: vec![
            Some(ChunkState::Trailer("Farm: FarmValue".into())),
            Some(ChunkState::Trailer("Farm:FarmValue".into())),
            Some(ChunkState::Trailer("Farm:FarmValue".into())),
            None,
        ],
        content: &[
            "Farm: FarmValue\r\n",
            "Farm:FarmValue\r\n",
            "Farm:FarmValue\n\n",
            "\r\n",
        ][..],
    }];

    for sample in test_cases {
        let chunks: Result<Vec<Option<ChunkState>>, ChunkStateError> = sample
            .content
            .iter()
            .map(|t| ChunkState::parse_http_trailer_chunk(t.as_bytes()))
            .collect();

        assert!(chunks.is_ok());
        assert_eq!(chunks.unwrap(), sample.expected);
    }
}

#[test]
#[traced_test]
fn test_chunk_state_parse_http_chunk_code() {
    let test_cases: Vec<ChunkSample> = vec![
        ChunkSample {
            expected: vec![
                ChunkState::Chunk(7, "7".into(), None),
                ChunkState::Chunk(17, "11".into(), None),
                ChunkState::LastChunk,
            ],
            content: &["7\r\nMozilla\r\n", "11\r\nDeveloper Network\r\n", "0\r\n"][..],
        },
        ChunkSample {
            expected: vec![
                ChunkState::Chunk(
                    5,
                    "5".into(),
                    Some(vec![
                        (" comment".into(), Some("\"first chunk\"".into())),
                        ("day".into(), Some("1".into())),
                    ]),
                ),
                ChunkState::Chunk(
                    5,
                    "5".into(),
                    Some(vec![
                        (" comment".into(), Some("\"first chunk\"".into())),
                        (" age".into(), Some("1".into())),
                    ]),
                ),
                ChunkState::Chunk(
                    5,
                    "5".into(),
                    Some(vec![(" comment".into(), Some("\"first chunk\"".into()))]),
                ),
                ChunkState::Chunk(
                    5,
                    "5".into(),
                    Some(vec![(" comment".into(), Some("\"second chunk\"".into()))]),
                ),
                ChunkState::Chunk(
                    5,
                    "5".into(),
                    Some(vec![(" name".into(), Some("second".into()))]),
                ),
                ChunkState::Chunk(5, "5".into(), Some(vec![(" ranger".into(), None)])),
                ChunkState::LastChunk,
            ],
            content: &[
                "5; comment=\"first chunk\";day=1\r\nhello",
                "5; comment=\"first chunk\"; age=1\r\nhello",
                "5; comment=\"first chunk\"\r\nhello",
                "5; comment=\"second chunk\"\r\nworld",
                "5; name=second\r\nworld",
                "5; ranger\r\nworld",
                "0\r\n",
            ][..],
        },
    ];

    for sample in test_cases {
        let chunks: Result<Vec<ChunkState>, ChunkStateError> = sample
            .content
            .iter()
            .map(|t| ChunkState::parse_http_chunk(t.as_bytes()))
            .collect();

        dbg!(&chunks);
        assert!(chunks.is_ok());
        assert_eq!(chunks.unwrap(), sample.expected);
    }
}

#[test]
fn test_chunk_state_octet_string_parsing() {
    assert!(matches!(
        ChunkState::try_new("0".into(), None),
        Ok(ChunkState::Chunk(0, _, _))
    ));
    assert!(matches!(
        ChunkState::try_new("12".into(), None),
        Ok(ChunkState::Chunk(18, _, _))
    ));
    assert!(matches!(
        ChunkState::try_new("35".into(), None),
        Ok(ChunkState::Chunk(53, _, _))
    ));
    assert!(matches!(
        ChunkState::try_new("3086d".into(), None),
        Ok(ChunkState::Chunk(198765, _, _))
    ));
}
