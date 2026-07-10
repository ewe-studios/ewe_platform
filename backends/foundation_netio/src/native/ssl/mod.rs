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

#[cfg(all(not(feature = "ssl-rustls"), not(feature = "ssl-openssl"), feature = "ssl-native-tls"))]
pub mod native_ttls;

#[cfg(all(not(feature = "ssl-rustls"), not(feature = "ssl-openssl"), feature = "ssl-native-tls"))]
pub use self::native_ttls::NativeTlsAcceptor as SSLAcceptor;

#[cfg(all(not(feature = "ssl-rustls"), not(feature = "ssl-openssl"), feature = "ssl-native-tls"))]
pub use self::native_ttls::NativeTlsConnector as SSLConnector;

#[cfg(all(not(feature = "ssl-rustls"), not(feature = "ssl-openssl"), feature = "ssl-native-tls"))]
pub use self::native_ttls::NativeTlsStream as ClientSSLStream;

#[cfg(all(not(feature = "ssl-rustls"), not(feature = "ssl-openssl"), feature = "ssl-native-tls"))]
pub use self::native_ttls::NativeTlsStream as ServerSSLStream;
