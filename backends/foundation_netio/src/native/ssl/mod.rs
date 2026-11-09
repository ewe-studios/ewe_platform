//! Taken from the tiny-http project <https://github.com/tiny-http/tiny-http>/

#![cfg(not(target_family = "wasm"))]

#[cfg(not(any(
    feature = "ssl-rustls",
    feature = "ssl-openssl",
    feature = "ssl-native-tls"
)))]
mod nossl;

#[cfg(not(any(
    feature = "ssl-rustls",
    feature = "ssl-openssl",
    feature = "ssl-native-tls"
)))]
#[allow(unused_imports)]
pub use nossl::*;

mod config;
pub use config::*;

// TLS backend priority: ssl-rustls > ssl-openssl > ssl-native-tls.
// When --all-features enables multiple backends, the primary (rustls) wins.
// This avoids compile_error! conflicts while still allowing explicit single-backend selection.

// Rustls wins when both backends are on, so the `openssl` module is not compiled and
// cargo reports the `openssl` dependency as unused. Naming the crate in that build only
// keeps the manifest clean; each backend alone stays clean.
#[cfg(all(feature = "ssl-openssl", feature = "ssl-rustls"))]
use openssl as _;

#[cfg(all(feature = "ssl-openssl", not(feature = "ssl-rustls")))]
pub mod openssl;

#[cfg(all(
    feature = "ssl-openssl",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-native-tls")
))]
pub use self::openssl::OpenSslAcceptor as SSLAcceptor;

#[cfg(all(
    feature = "ssl-openssl",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-native-tls")
))]
pub use self::openssl::OpenSslConnector as SSLConnector;

#[cfg(all(
    feature = "ssl-openssl",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-native-tls")
))]
pub use self::openssl::SplitOpenSslStream as ServerSSLStream;

#[cfg(all(
    feature = "ssl-openssl",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-native-tls")
))]
pub use self::openssl::SplitOpenSslStream as ClientSSLStream;

#[cfg(feature = "ssl-rustls")]
pub mod rustls;

#[cfg(feature = "ssl-rustls")]
pub use self::rustls::RustTlsClientStream as ClientSSLStream;

#[cfg(feature = "ssl-rustls")]
pub use self::rustls::RustTlsServerStream as ServerSSLStream;

#[cfg(feature = "ssl-rustls")]
pub use self::rustls::RustlsAcceptor as SSLAcceptor;

#[cfg(feature = "ssl-rustls")]
pub use self::rustls::RustlsConnector as SSLConnector;

#[cfg(all(
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl"),
    feature = "ssl-native-tls"
))]
pub mod native_ttls;

#[cfg(all(
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl"),
    feature = "ssl-native-tls"
))]
pub use self::native_ttls::NativeTlsAcceptor as SSLAcceptor;

#[cfg(all(
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl"),
    feature = "ssl-native-tls"
))]
pub use self::native_ttls::NativeTlsConnector as SSLConnector;

#[cfg(all(
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl"),
    feature = "ssl-native-tls"
))]
pub use self::native_ttls::NativeTlsStream as ClientSSLStream;

#[cfg(all(
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl"),
    feature = "ssl-native-tls"
))]
pub use self::native_ttls::NativeTlsStream as ServerSSLStream;

// ── Shared helpers for the TLS connect paths ────────────────────────────────

/// Connect a plain TCP stream on a non-blocking socket and leave it
/// non-blocking, so the TLS handshake can be driven by readiness.
///
/// The synchronous constructors then wait on the same readiness a valtron task
/// parks on, instead of handing a worker to a blocking `connect(2)` (issue
/// #277).
///
/// # Errors
/// The connect failed or the timeout expired.
pub(crate) fn connect_plain(
    addr: core::net::SocketAddr,
    timeout: Option<std::time::Duration>,
) -> std::io::Result<std::net::TcpStream> {
    #[cfg(unix)]
    {
        crate::native::connection::connect_tcp_nonblocking(addr, timeout)
    }
    #[cfg(not(unix))]
    {
        let stream = match timeout {
            Some(timeout) => std::net::TcpStream::connect_timeout(&addr, timeout)?,
            None => std::net::TcpStream::connect(addr)?,
        };
        stream.set_nonblocking(true)?;
        Ok(stream)
    }
}

/// The connect timeout an endpoint carries, if any.
pub(crate) fn endpoint_timeout<I: Clone>(
    endpoint: &crate::native::connection::Endpoint<I>,
) -> Option<std::time::Duration> {
    use crate::native::connection::{Endpoint, EndpointConfig};
    match endpoint {
        Endpoint::WithDefault(config) | Endpoint::WithIdentity(config, _) => match config {
            EndpointConfig::WithTimeout(_, timeout) => Some(*timeout),
            EndpointConfig::NoTimeout(_) => None,
        },
    }
}
