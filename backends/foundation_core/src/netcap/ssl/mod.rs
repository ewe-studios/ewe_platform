//! Taken from the tiny-http project https://github.com/tiny-http/tiny-http/

#![cfg(not(target_arch = "wasm32"))]

#[cfg(feature = "ssl-openssl")]
pub mod openssl;
#[cfg(feature = "ssl-openssl")]
pub use self::openssl::OpenSslAcceptor as SSLAcceptor;
#[cfg(feature = "ssl-openssl")]
pub use self::openssl::OpenSslConnector as SSLConnector;
#[cfg(feature = "ssl-openssl")]
pub use self::openssl::SplitOpenSslStream as ServerSSLStream;
#[cfg(feature = "ssl-openssl")]
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

#[cfg(feature = "ssl-native-tls")]
pub mod native_tls;
#[cfg(feature = "ssl-native-tls")]
pub use self::native_tls::NativeTlsAcceptor as SSLAcceptor;
#[cfg(feature = "ssl-native-tls")]
pub use self::native_tls::NativeTlsConnector as SSLConnector;
#[cfg(feature = "ssl-native-tls")]
pub use self::native_tls::NativeTlsStream as ClientSSLStream;
#[cfg(feature = "ssl-native-tls")]
pub use self::native_tls::NativeTlsStream as ServerSSLStream;
