//! Decoding Docker's multiplexed stdout/stderr stream.
//!
//! WHY: `GET /containers/{id}/logs`, `POST /containers/{id}/attach` and
//! `POST /exec/{id}/start` answer (without a TTY) with stdout and stderr
//! interleaved in one body, each piece behind an 8-byte header. The bytes have
//! to be split back into frames as they arrive.
//!
//! WHAT: [`decoder::LogFrameDecoder`] and the frames it yields,
//! [`decoder::LogOutput`].
//!
//! HOW: The requests themselves go through [`DockerClient`], whose methods call
//! the generated `*_request` functions and hand the body back as a
//! [`LogStream`] — that stream feeds the body to a [`decoder::LogFrameDecoder`]
//! chunk by chunk. JSON-lines bodies (events, stats, build/pull/push progress)
//! are parsed by `foundation_deployment`'s `JsonLinesStream`, wrapped in
//! [`EventStream`], [`StatsStream`] and [`ProgressStream`].
//!
//! [`DockerClient`]: crate::client::DockerClient
//! [`LogStream`]: crate::client::body::LogStream
//! [`EventStream`]: crate::client::body::EventStream
//! [`StatsStream`]: crate::client::body::StatsStream
//! [`ProgressStream`]: crate::client::body::ProgressStream

pub mod decoder;
