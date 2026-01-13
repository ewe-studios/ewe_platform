//! Taken from the tiny-http project <https://github.com/tiny-http/tiny-http>/

#![cfg(not(target_arch = "wasm32"))]

#[cfg(all(feature = "ssl-openssl", not(feature="ssl-rustls"), not(feature="ssl-native-tls")))]
pub mod openssl;
#[cfg(all(feature = "ssl-openssl", not(feature="ssl-rustls"), not(feature="ssl-native-tls")))]
pub use self::openssl::OpenSslAcceptor as SSLAcceptor;
#[cfg(all(feature = "ssl-openssl", not(feature="ssl-rustls"), not(feature="ssl-native-tls")))]
pub use self::openssl::OpenSslConnector as SSLConnector;
#[cfg(all(feature = "ssl-openssl", not(feature="ssl-rustls"), not(feature="ssl-native-tls")))]
pub use self::openssl::SplitOpenSslStream as ServerSSLStream;
#[cfg(all(feature = "ssl-openssl", not(feature="ssl-rustls"), not(feature="ssl-native-tls")))]
pub use self::openssl::SplitOpenSslStream as ClientSSLStream;

#[cfg(all(feature = "ssl-rustls", not(feature="ssl-openssl"), not(feature="ssl-native-tls")))]
pub mod rustls;
#[cfg(all(feature = "ssl-rustls", not(feature="ssl-openssl"), not(feature="ssl-native-tls")))]
pub use self::rustls::RustTlsClientStream as ClientSSLStream;
#[cfg(all(feature = "ssl-rustls", not(feature="ssl-openssl"), not(feature="ssl-native-tls")))]
pub use self::rustls::RustTlsServerStream as ServerSSLStream;
#[cfg(all(feature = "ssl-rustls", not(feature="ssl-openssl"), not(feature="ssl-native-tls")))]
pub use self::rustls::RustlsAcceptor as SSLAcceptor;
#[cfg(all(feature = "ssl-rustls", not(feature="ssl-openssl"), not(feature="ssl-native-tls")))]
pub use self::rustls::RustlsConnector as SSLConnector;

#[cfg(all(feature = "ssl-native-tls", not(feature="ssl-rustls"), not(feature="ssl-openssl")))]
pub mod native_ttls;
#[cfg(all(feature = "ssl-native-tls", not(feature="ssl-rustls"), not(feature="ssl-openssl")))]
pub use self::native_ttls::NativeTlsAcceptor as SSLAcceptor;
#[cfg(all(feature = "ssl-native-tls", not(feature="ssl-rustls"), not(feature="ssl-openssl")))]
pub use self::native_ttls::NativeTlsConnector as SSLConnector;
#[cfg(all(feature = "ssl-native-tls", not(feature="ssl-rustls"), not(feature="ssl-openssl")))]
pub use self::native_ttls::NativeTlsStream as ClientSSLStream;
#[cfg(all(feature = "ssl-native-tls", not(feature="ssl-rustls"), not(feature="ssl-openssl")))]
pub use self::native_ttls::NativeTlsStream as ServerSSLStream;
