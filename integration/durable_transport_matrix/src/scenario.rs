//! The Given/When/Then scenario list of the transport matrix.
//!
//! This list is the specification. Every driver runs it unchanged. A new
//! transport adds one driver; it does not add a scenario.

use crate::driver::{Driver, Observation, Producer};

/// The stream the main scenarios use.
pub const STREAM: &str = "matrix/scenario";
/// The fork source.
pub const SOURCE: &str = "matrix/source";
/// The fork child.
pub const CHILD: &str = "matrix/child";
/// A stream that no scenario creates.
pub const MISSING: &str = "matrix/missing";
/// The media type of every scenario stream.
pub const CONTENT_TYPE: &str = "application/octet-stream";
/// The sliding time to live (TTL) of the TTL scenario, in milliseconds.
///
/// The HTTP surface carries `Stream-TTL` in whole seconds, so a whole second
/// keeps every transport on the same deadline.
pub const TTL_MS: u64 = 1_000;
/// How long the TTL scenario waits before it reads the expired stream.
///
/// The wait is short and bounded. The margin covers scheduler delay.
pub const TTL_WAIT_MS: u64 = 1_500;

/// One behaviour of the protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// Given a stream, when it is created and appended, then a catch-up read
    /// returns every byte.
    CreateAppendRead,
    /// Given two appends, when it reads from the first next offset, then only
    /// the second chunk arrives.
    ReadFromFirstNextOffset,
    /// Given a closed stream, when it reads and appends, then the read reports
    /// closed and the append is refused.
    CloseReadAndRefusedAppend,
    /// Given a source at its tail, when a fork appends, then the fork reads the
    /// prefix and its own chunk.
    ForkAtTail,
    /// Given a missing stream, when it reads, then the read reports not found.
    ReadMissing,
    /// Given a producer, when it retries the same sequence, then the stream
    /// holds one chunk and the append reports a duplicate.
    ProducerRetry,
    /// Given a stream sequence, when it repeats or regresses, then the store
    /// refuses and accepts a new sequence.
    StreamSeq,
    /// Given a stream with a TTL, when the TTL passes, then a read reports not
    /// found.
    TtlExpiry,
}

impl Scenario {
    /// Every scenario, in the order the specification lists them.
    #[must_use]
    pub fn all() -> [Self; 8] {
        [
            Self::CreateAppendRead,
            Self::ReadFromFirstNextOffset,
            Self::CloseReadAndRefusedAppend,
            Self::ForkAtTail,
            Self::ReadMissing,
            Self::ProducerRetry,
            Self::StreamSeq,
            Self::TtlExpiry,
        ]
    }

    /// The Given/When/Then name of the scenario.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::CreateAppendRead => {
                "given_a_stream_when_it_is_created_and_appended_then_a_catch_up_read_returns_every_byte"
            }
            Self::ReadFromFirstNextOffset => {
                "given_two_appends_when_it_reads_from_the_first_next_offset_then_only_the_second_chunk_arrives"
            }
            Self::CloseReadAndRefusedAppend => {
                "given_a_closed_stream_when_it_reads_and_appends_then_the_read_reports_closed_and_the_append_is_refused"
            }
            Self::ForkAtTail => {
                "given_a_source_at_its_tail_when_a_fork_appends_then_the_fork_reads_the_prefix_and_its_own_chunk"
            }
            Self::ReadMissing => {
                "given_a_missing_stream_when_it_reads_then_the_read_reports_not_found"
            }
            Self::ProducerRetry => {
                "given_a_producer_when_it_retries_the_same_sequence_then_the_stream_holds_one_chunk_and_reports_a_duplicate"
            }
            Self::StreamSeq => {
                "given_a_stream_seq_when_it_repeats_or_regresses_then_the_store_refuses_and_accepts_a_new_sequence"
            }
            Self::TtlExpiry => {
                "given_a_stream_with_a_ttl_when_the_ttl_passes_then_a_read_reports_not_found"
            }
        }
    }
}

/// Run one scenario on one driver and log every step.
pub async fn run_scenario<D: Driver>(driver: &mut D, scenario: Scenario) -> Vec<Observation> {
    match scenario {
        Scenario::CreateAppendRead => create_append_read(driver).await,
        Scenario::ReadFromFirstNextOffset => read_from_first_next_offset(driver).await,
        Scenario::CloseReadAndRefusedAppend => close_read_and_refused_append(driver).await,
        Scenario::ForkAtTail => fork_at_tail(driver).await,
        Scenario::ReadMissing => read_missing(driver).await,
        Scenario::ProducerRetry => producer_retry(driver).await,
        Scenario::StreamSeq => stream_seq(driver).await,
        Scenario::TtlExpiry => ttl_expiry(driver).await,
    }
}

/// Create, append two chunks, then run a catch-up read.
async fn create_append_read<D: Driver>(driver: &mut D) -> Vec<Observation> {
    let mut log = Vec::new();
    log.push(driver.create(STREAM, CONTENT_TYPE, None).await.observation);
    for chunk in [b"hello ".as_slice(), b"world".as_slice()] {
        log.push(
            driver
                .append(STREAM, chunk, CONTENT_TYPE, None, None, false)
                .await
                .observation,
        );
    }
    log.push(driver.read(STREAM, None).await.observation);
    log
}

/// Append twice, then read from the first append's next offset.
async fn read_from_first_next_offset<D: Driver>(driver: &mut D) -> Vec<Observation> {
    let mut log = Vec::new();
    log.push(driver.create(STREAM, CONTENT_TYPE, None).await.observation);
    let first = driver
        .append(STREAM, b"first", CONTENT_TYPE, None, None, false)
        .await;
    let next = first.next_offset.clone();
    log.push(first.observation);
    log.push(
        driver
            .append(STREAM, b"second", CONTENT_TYPE, None, None, false)
            .await
            .observation,
    );
    log.push(driver.read(STREAM, next.as_deref()).await.observation);
    log
}

/// Close a stream, read it, then retry an append.
async fn close_read_and_refused_append<D: Driver>(driver: &mut D) -> Vec<Observation> {
    let mut log = Vec::new();
    log.push(driver.create(STREAM, CONTENT_TYPE, None).await.observation);
    log.push(
        driver
            .append(STREAM, b"a", CONTENT_TYPE, None, None, false)
            .await
            .observation,
    );
    log.push(driver.close(STREAM, b"", CONTENT_TYPE).await.observation);
    log.push(driver.read(STREAM, None).await.observation);
    log.push(
        driver
            .append(STREAM, b"more", CONTENT_TYPE, None, None, false)
            .await
            .observation,
    );
    log
}

/// Fork a source at its tail, append to the fork, then read both.
async fn fork_at_tail<D: Driver>(driver: &mut D) -> Vec<Observation> {
    let mut log = Vec::new();
    log.push(driver.create(SOURCE, CONTENT_TYPE, None).await.observation);
    let appended = driver
        .append(SOURCE, b"0123", CONTENT_TYPE, None, None, false)
        .await;
    let at = appended.next_offset.clone();
    log.push(appended.observation);
    log.push(driver.fork(CHILD, SOURCE, at.as_deref()).await.observation);
    log.push(
        driver
            .append(CHILD, b"45", CONTENT_TYPE, None, None, false)
            .await
            .observation,
    );
    log.push(driver.read(CHILD, None).await.observation);
    log.push(driver.read(SOURCE, None).await.observation);
    log
}

/// Read a stream that no step created.
async fn read_missing<D: Driver>(driver: &mut D) -> Vec<Observation> {
    vec![driver.read(MISSING, None).await.observation]
}

/// Append with a producer, then retry the same sequence.
async fn producer_retry<D: Driver>(driver: &mut D) -> Vec<Observation> {
    let producer = Producer {
        id: "matrix-producer".to_string(),
        epoch: 0,
        seq: 0,
    };
    let mut log = Vec::new();
    log.push(driver.create(STREAM, CONTENT_TYPE, None).await.observation);
    for _ in 0..2 {
        log.push(
            driver
                .append(STREAM, b"once", CONTENT_TYPE, Some(&producer), None, false)
                .await
                .observation,
        );
    }
    log.push(driver.read(STREAM, None).await.observation);
    log
}

/// Append with `Stream-Seq`: a repeat and a regression are refused, then a
/// larger sequence is accepted.
async fn stream_seq<D: Driver>(driver: &mut D) -> Vec<Observation> {
    let mut log = Vec::new();
    log.push(driver.create(STREAM, CONTENT_TYPE, None).await.observation);
    for (chunk, seq) in [
        (b"a".as_slice(), "b"),
        (b"b".as_slice(), "b"),
        (b"c".as_slice(), "a"),
        (b"d".as_slice(), "c"),
    ] {
        log.push(
            driver
                .append(STREAM, chunk, CONTENT_TYPE, None, Some(seq), false)
                .await
                .observation,
        );
    }
    log.push(driver.read(STREAM, None).await.observation);
    log
}

/// Create with a short TTL, append, wait for the deadline, then read.
async fn ttl_expiry<D: Driver>(driver: &mut D) -> Vec<Observation> {
    let mut log = Vec::new();
    log.push(
        driver
            .create(STREAM, CONTENT_TYPE, Some(TTL_MS))
            .await
            .observation,
    );
    log.push(
        driver
            .append(STREAM, b"ttl", CONTENT_TYPE, None, None, false)
            .await
            .observation,
    );
    driver
        .wait(core::time::Duration::from_millis(TTL_WAIT_MS))
        .await;
    log.push(driver.read(STREAM, None).await.observation);
    log
}
