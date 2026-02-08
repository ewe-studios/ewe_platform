//! Taken from the tiny-http project <https://github.com/tiny-http/tiny-http>/

#![cfg(not(target_arch = "wasm32"))]

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

// Compile-time checks to ensure only one TLS backend is enabled
#[cfg(all(feature = "ssl-rustls", feature = "ssl-openssl"))]
compile_error!("Cannot enable both `ssl-rustls` and `ssl-openssl`. Choose one TLS backend.");

#[cfg(all(feature = "ssl-rustls", feature = "ssl-native-tls"))]
compile_error!("Cannot enable both `ssl-rustls` and `ssl-native-tls`. Choose one TLS backend.");

#[cfg(all(feature = "ssl-openssl", feature = "ssl-native-tls"))]
compile_error!("Cannot enable both `ssl-openssl` and `ssl-native-tls`. Choose one TLS backend.");

#[cfg(all(
    feature = "ssl-rustls",
    feature = "ssl-openssl",
    feature = "ssl-native-tls"
))]
compile_error!("Cannot enable all three TLS backends simultaneously. Choose only one: ssl-rustls, ssl-openssl, or ssl-native-tls.");

#[cfg(all(
    feature = "ssl-openssl",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-native-tls")
))]
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

#[cfg(all(
    feature = "ssl-rustls",
    not(feature = "ssl-openssl"),
    not(feature = "ssl-native-tls")
))]
pub mod rustls;
#[cfg(all(
    feature = "ssl-rustls",
    not(feature = "ssl-openssl"),
    not(feature = "ssl-native-tls")
))]
pub use self::rustls::RustTlsClientStream as ClientSSLStream;
#[cfg(all(
    feature = "ssl-rustls",
    not(feature = "ssl-openssl"),
    not(feature = "ssl-native-tls")
))]
pub use self::rustls::RustTlsServerStream as ServerSSLStream;
#[cfg(all(
    feature = "ssl-rustls",
    not(feature = "ssl-openssl"),
    not(feature = "ssl-native-tls")
))]
pub use self::rustls::RustlsAcceptor as SSLAcceptor;
#[cfg(all(
    feature = "ssl-rustls",
    not(feature = "ssl-openssl"),
    not(feature = "ssl-native-tls")
))]
pub use self::rustls::RustlsConnector as SSLConnector;

#[cfg(all(
    feature = "ssl-native-tls",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl")
))]
pub mod native_ttls;
#[cfg(all(
    feature = "ssl-native-tls",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl")
))]
pub use self::native_ttls::NativeTlsAcceptor as SSLAcceptor;
#[cfg(all(
    feature = "ssl-native-tls",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl")
))]
pub use self::native_ttls::NativeTlsConnector as SSLConnector;
#[cfg(all(
    feature = "ssl-native-tls",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl")
))]
pub use self::native_ttls::NativeTlsStream as ClientSSLStream;
#[cfg(all(
    feature = "ssl-native-tls",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl")
))]
pub use self::native_ttls::NativeTlsStream as ServerSSLStream;
