//! WebTransport (draft-ietf-webtrans-http3) over sans-I/O QUIC + HTTP/3 (spec-55, F06).
//!
//! WHY: A QUIC-native transport with datagrams and unbundled streams — no head-of-line
//! blocking, unlike WebSocket. The WG browser relay (F07) uses it as an optimization
//! path behind the always-available WebSocket relay.
//!
//! WHAT: [`WtSession`] — one WebTransport session atop an H3 connection: open/accept
//! bidirectional and unidirectional streams, send/receive unreliable datagrams, close.
//! [`WtAcceptor`] — server side: accept incoming WT sessions from h3 Extended CONNECT.
//! [`WtConnector`] — client side: initiate a WT session.
//!
//! HOW: All methods are progress-returning (`Stream`-based). No tokio — the driving
//! valtron task can park on `Pending`/`Wait`. Built on the existing quic traits +
//! http3 substrate.

pub mod proto;
pub mod session;

pub use proto::{CapsuleType, WtProtocolError};
pub use session::{WtAcceptor, WtConnector, WtSession, WtStreamError};
