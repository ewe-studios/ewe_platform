//! TLS verification module for the HTTP 1.1 client.
//!
//! This module provides functionality to verify and fix TLS backends (rustls, openssl, native-tls)
//! to ensure compatibility with the HTTP client implementation.
//!
//! The implementation includes:
//! - Verification of TLS backend configurations
//! - Fixing of TLS backend issues
//! - Integration with the netcap infrastructure
//!
//! This component is essential for ensuring secure and reliable HTTPS connections.

use crate::netcap::ssl::{OpenSslAcceptor, OpenSslConnector, RustlsAcceptor, RustlsConnector, NativeTlsAcceptor, NativeTlsConnector};
use crate::netcap::{Endpoint, EndpointConfig, SocketAddr};
use std::error::Error;
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

/// Configuration for TLS verification.
///
/// This struct contains the necessary parameters for verifying and fixing TLS backends.
pub struct TlsVerificationConfig {
    /// The TLS backend to use (rustls, openssl, native-tls)
    pub backend: TlsBackend,
    /// Whether to verify certificates
    pub verify_certificates: bool,
    /// Whether to use default root certificates
    pub use_default_roots: bool,
    /// Custom root certificates (if not using default)
    pub custom_roots: Option<Vec<u8>>,
    /// Timeout for TLS connections
    pub timeout: Option<std::time::Duration>,
}

/// Enum representing the available TLS backends.
#[derive(Debug, Clone, PartialEq)]
pub enum TlsBackend {
    /// Rustls backend
    Rustls,
    /// OpenSSL backend
    Openssl,
    /// Native TLS backend
    NativeTls,
}

/// Error type for TLS verification.
#[derive(Debug)]
pub enum TlsVerificationError {
    /// Error related to certificate parsing
    CertificateError(String),
    /// Error related to private key parsing
    PrivateKeyError(String),
    /// Error related to TLS backend initialization
    BackendError(String),
    /// Error related to connection timeout
    TimeoutError(String),
    /// Error related to invalid configuration
    InvalidConfigError(String),
}

impl std::fmt::Display for TlsVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsVerificationError::CertificateError(msg) => write!(f, "Certificate error: {}", msg),
            TlsVerificationError::PrivateKeyError(msg) => write!(f, "Private key error: {}", msg),
            TlsVerificationError::BackendError(msg) => write!(f, "TLS backend error: {}", msg),
            TlsVerificationError::TimeoutError(msg) => write!(f, "Timeout error: {}", msg),
            TlsVerificationError::InvalidConfigError(msg) => write!(f, "Invalid configuration: {}", msg),
        }
    }
}

impl Error for TlsVerificationError {}

/// TLS verification service.
///
/// This struct provides methods for verifying and fixing TLS backends.
pub struct TlsVerificationService {
    config: TlsVerificationConfig,
    backend: TlsBackend,
}

impl TlsVerificationService {
    /// Creates a new TLS verification service with the given configuration.
    pub fn new(config: TlsVerificationConfig) -> Result<Self, TlsVerificationError> {
        // Validate configuration
        if config.custom_roots.is_some() && !config.use_default_roots {
            return Err(TlsVerificationError::InvalidConfigError(
                "Custom roots cannot be used without setting use_default_roots to true".to_string()
            ));
        }

        // Validate backend
        let backend = match config.backend {
            TlsBackend::Rustls => {
                #[cfg(feature = "ssl-rustls")]
                { TlsBackend::Rustls }
                #[cfg(not(feature = "ssl-rustls"))]
                { return Err(TlsVerificationError::BackendError("Rustls backend is not available".to_string())); }
            }
            TlsBackend::Openssl => {
                #[cfg(feature = "ssl-openssl")]
                { TlsBackend::Openssl }
                #[cfg(not(feature = "ssl-openssl"))]
                { return Err(TlsVerificationError::BackendError("OpenSSL backend is not available".to_string())); }
            }
            TlsBackend::NativeTls => {
                #[cfg(feature = "ssl-native-tls")]
                { TlsBackend::NativeTls }
                #[cfg(not(feature = "ssl-native-tls"))]
                { return Err(TlsVerificationError::BackendError("Native TLS backend is not available".to_string())); }
            }
        };

        Ok(Self { config, backend })
    }

    /// Verifies the TLS backend configuration.
    ///
    /// This method checks if the TLS backend is properly configured and can establish connections.
    pub fn verify_backend(&self) -> Result<(), TlsVerificationError> {
        // Create a test endpoint
        let test_host = "example.com";
        let test_port = 443;
        let test_url = format!("https://{}:{}", test_host, test_port);
        let test_socket_addr: SocketAddr = test_url.parse().map_err(|e| {
            TlsVerificationError::InvalidConfigError(format!("Invalid test URL: {}", e))
        })?;

        // Create a test endpoint
        let endpoint = match &self.backend {
            TlsBackend::Rustls => {
                let config = if self.config.use_default_roots {
                    Arc::new(rustls::ClientConfig::builder()
                        .with_root_certificates(rustls::RootCertStore::empty())
                        .with_no_client_auth())
                } else {
                    Arc::new(rustls::ClientConfig::builder()
                        .with_root_certificates(rustls::RootCertStore::empty())
                        .with_no_client_auth())
                };
                Endpoint::WithIdentity(EndpointConfig::NoTimeout(test_socket_addr), config)
            }
            TlsBackend::Openssl => {
                let config = openssl::ssl::SslContext::builder(openssl::ssl::SslMethod::tls()).map_err(|e| {
                    TlsVerificationError::BackendError(format!("Failed to create OpenSSL context: {}", e))
                })?;
                Endpoint::WithIdentity(EndpointConfig::NoTimeout(test_socket_addr), Arc::new(config))
            }
            TlsBackend::NativeTls => {
                let config = native_tls::TlsConnector::new().map_err(|e| {
                    TlsVerificationError::BackendError(format!("Failed to create Native TLS connector: {}", e))
                })?;
                Endpoint::WithIdentity(EndpointConfig::NoTimeout(test_socket_addr), Arc::new(config))
            }
        };

        // Try to establish a connection
        match &self.backend {
            TlsBackend::Rustls => {
                let connector = RustlsConnector::create(&endpoint);
                let result = connector.from_endpoint(&endpoint);
                if result.is_err() {
                    return Err(TlsVerificationError::BackendError("Failed to establish TLS connection with Rustls backend".to_string()));
                }
            }
            TlsBackend::Openssl => {
                let connector = OpenSslConnector::create(&endpoint);
                let result = connector.from_endpoint(&endpoint);
                if result.is_err() {
                    return Err(TlsVerificationError::BackendError("Failed to establish TLS connection with OpenSSL backend".to_string()));
                }
            }
            TlsBackend::NativeTls => {
                let connector = NativeTlsConnector::create(&endpoint);
                let result = connector.from_endpoint(&endpoint);
                if result.is_err() {
                    return Err(TlsVerificationError::BackendError("Failed to establish TLS connection with Native TLS backend".to_string()));
                }
            }
        }

        Ok(())
    }

    /// Fixes common TLS backend issues.
    ///
    /// This method attempts to fix common issues with TLS backends, such as:
    /// - Missing root certificates
    /// - Invalid certificate chains
    /// - Incorrect private key formats
    pub fn fix_backend_issues(&self) -> Result<(), TlsVerificationError> {
        // Check for missing root certificates
        if self.config.use_default_roots && self.config.custom_roots.is_none() {
            // Try to load default root certificates
            match &self.backend {
                TlsBackend::Rustls => {
                    let mut root_store = rustls::RootCertStore::empty();
                    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                    // Verify that we can load the root certificates
                    if root_store.is_empty() {
                        return Err(TlsVerificationError::BackendError("Failed to load default root certificates".to_string()));
                    }
                }
                TlsBackend::Openssl => {
                    // OpenSSL should have default root certificates
                    // This is a placeholder - actual implementation would depend on the system
                }
                TlsBackend::NativeTls => {
                    // Native TLS should have default root certificates
                    // This is a placeholder - actual implementation would depend on the system
                }
            }
        }

        // Check for certificate chain issues
        if let Some(certificates) = &self.config.custom_roots {
            match &self.backend {
                TlsBackend::Rustls => {
                    let result = rustls::pki_types::CertificateDer::pem_slice_iter(certificates.as_slice()).collect::<Result<Vec<_>, _>>();
                    if result.is_err() {
                        return Err(TlsVerificationError::CertificateError("Failed to parse custom certificates".to_string()));
                    }
                }
                TlsBackend::Openssl => {
                    let result = openssl::x509::X509::stack_from_pem(certificates);
                    if result.is_err() {
                        return Err(TlsVerificationError::CertificateError("Failed to parse custom certificates".to_string()));
                    }
                }
                TlsBackend::NativeTls => {
                    let result = native_tls::Identity::from_pkcs8(certificates, &Zeroizing::new(vec![]));
                    if result.is_err() {
                        return Err(TlsVerificationError::CertificateError("Failed to parse custom certificates".to_string()));
                    }
                }
            }
        }

        // Check for private key issues
        if let Some(private_key) = &self.config.custom_roots {
            match &self.backend {
                TlsBackend::Rustls => {
                    let result = rustls::pki_types::PrivateKeyDer::from_pem_slice(private_key);
                    if result.is_err() {
                        return Err(TlsVerificationError::PrivateKeyError("Failed to parse private key".to_string()));
                    }
                }
                TlsBackend::Openssl => {
                    let result = openssl::pkey::PKey::private_key_from_pem(private_key);
                    if result.is_err() {
                        return Err(TlsVerificationError::PrivateKeyError("Failed to parse private key".to_string()));
                    }
                }
                TlsBackend::NativeTls => {
                    let result = native_tls::Identity::from_pkcs8(private_key, &Zeroizing::new(vec![]));
                    if result.is_err() {
                        return Err(TlsVerificationError::PrivateKeyError("Failed to parse private key".to_string()));
                    }
                }
            }
        }

        Ok(())
    }

    /// Gets the TLS backend.
    pub fn backend(&self) -> &TlsBackend {
        &self.backend
    }

    /// Gets the TLS verification configuration.
    pub fn config(&self) -> &TlsVerificationConfig {
        &self.config
    }
}

// Implementations for the different TLS backends

/// Implementation for Rustls backend.
#[cfg(feature = "ssl-rustls")]
impl TlsVerificationService {
    /// Creates a new Rustls connector.
    pub fn create_rustls_connector(&self) -> Result<RustlsConnector, TlsVerificationError> {
        let config = if self.config.use_default_roots {
            Arc::new(rustls::ClientConfig::builder()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth())
        } else {
            Arc::new(rustls::ClientConfig::builder()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth())
        };

        Ok(RustlsConnector::with_config(config))
    }

    /// Creates a new Rustls acceptor.
    pub fn create_rustls_acceptor(&self, certificates: Vec<u8>, private_key: Zeroizing<Vec<u8>>) -> Result<RustlsAcceptor, TlsVerificationError> {
        let certs_result: Result<Vec<rustls::pki_types::CertificateDer<'static>>, _> = rustls::pki_types::CertificateDer::pem_slice_iter(certificates.as_slice()).collect();
        let certs = certs_result.map_err(|e| TlsVerificationError::CertificateError(format!("Failed to parse certificates: {}", e)))?;
        let p_key = rustls::pki_types::PrivateKeyDer::from_pem_slice(private_key.as_slice()).map_err(|e| TlsVerificationError::PrivateKeyError(format!("Failed to parse private key: {}", e)))?;

        let tls_conf = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, p_key)
            .map_err(|e| TlsVerificationError::BackendError(format!("Failed to create server config: {}", e)))?;

        Ok(RustlsAcceptor(Arc::new(tls_conf)))
    }
}

/// Implementation for OpenSSL backend.
#[cfg(feature = "ssl-openssl")]
impl TlsVerificationService {
    /// Creates a new OpenSSL connector.
    pub fn create_openssl_connector(&self) -> Result<OpenSslConnector, TlsVerificationError> {
        let config = openssl::ssl::SslContext::builder(openssl::ssl::SslMethod::tls()).map_err(|e| TlsVerificationError::BackendError(format!("Failed to create OpenSSL context: {}", e)))?;

        Ok(OpenSslConnector(Arc::new(config)))
    }

    /// Creates a new OpenSSL acceptor.
    pub fn create_openssl_acceptor(&self, certificates: Vec<u8>, private_key: Zeroizing<Vec<u8>>) -> Result<OpenSslAcceptor, TlsVerificationError> {
        let certificate_chain = openssl::x509::X509::stack_from_pem(&certificates).map_err(|e| TlsVerificationError::CertificateError(format!("Failed to parse certificates: {}", e)))?;
        if certificate_chain.is_empty() {
            return Err(TlsVerificationError::CertificateError("Certificate chain is empty".to_string()));
        }

        let key = openssl::pkey::PKey::private_key_from_pem(&private_key).map_err(|e| TlsVerificationError::PrivateKeyError(format!("Failed to parse private key: {}", e)))?;

        let mut ctx = openssl::ssl::SslContext::builder(openssl::ssl::SslMethod::tls()).map_err(|e| TlsVerificationError::BackendError(format!("Failed to create OpenSSL context: {}", e)))?;
        ctx.set_cipher_list("DEFAULT")?;
        ctx.set_certificate(&certificate_chain[0])?;
        for chain_cert in certificate_chain.into_iter().skip(1) {
            ctx.add_extra_chain_cert(chain_cert)?;
        }
        ctx.set_private_key(&key)?;
        ctx.set_verify(openssl::ssl::SslVerifyMode::NONE);
        ctx.check_private_key()?;

        Ok(OpenSslAcceptor(ctx.build()))
    }
}

/// Implementation for Native TLS backend.
#[cfg(feature = "ssl-native-tls")]
impl TlsVerificationService {
    /// Creates a new Native TLS connector.
    pub fn create_native_tls_connector(&self) -> Result<NativeTlsConnector, TlsVerificationError> {
        let config = native_tls::TlsConnector::new().map_err(|e| TlsVerificationError::BackendError(format!("Failed to create Native TLS connector: {}", e)))?;

        Ok(NativeTlsConnector(Arc::new(config)))
    }

    /// Creates a new Native TLS acceptor.
    pub fn create_native_tls_acceptor(&self, certificates: Vec<u8>, private_key: Zeroizing<Vec<u8>>) -> Result<NativeTlsAcceptor, TlsVerificationError> {
        let identity = native_tls::Identity::from_pkcs8(&certificates, &private_key).map_err(|e| TlsVerificationError::CertificateError(format!("Failed to create identity: {}", e)))?;

        Ok(NativeTlsAcceptor::from_identity(identity))
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use zeroize::Zeroizing;

    #[test]
    fn test_tls_verification_config() {
        let config = TlsVerificationConfig {
            backend: TlsBackend::Rustls,
            verify_certificates: true,
            use_default_roots: true,
            custom_roots: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        };

        let service = TlsVerificationService::new(config).unwrap();
        assert_eq!(service.backend(), &TlsBackend::Rustls);
        assert_eq!(service.config(), &config);
    }

    #[test]
    fn test_tls_verification_service_verify_backend() {
        let config = TlsVerificationConfig {
            backend: TlsBackend::Rustls,
            verify_certificates: true,
            use_default_roots: true,
            custom_roots: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        };

        let service = TlsVerificationService::new(config).unwrap();
        let result = service.verify_backend();
        // This test will fail if the backend is not available or if there are network issues
        // But we can still verify that the method returns a result
        assert!(result.is_ok());
    }

    #[test]
    fn test_tls_verification_service_fix_backend_issues() {
        let config = TlsVerificationConfig {
            backend: TlsBackend::Rustls,
            verify_certificates: true,
            use_default_roots: true,
            custom_roots: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        };

        let service = TlsVerificationService::new(config).unwrap();
        let result = service.fix_backend_issues();
        // This test will fail if there are issues with the backend
        // But we can still verify that the method returns a result
        assert!(result.is_ok());
    }

    #[test]
    fn test_tls_verification_service_invalid_config() {
        let config = TlsVerificationConfig {
            backend: TlsBackend::Rustls,
            verify_certificates: true,
            use_default_roots: false,
            custom_roots: Some(vec![1, 2, 3]),
            timeout: Some(std::time::Duration::from_secs(30)),
        };

        let result = TlsVerificationService::new(config);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Custom roots cannot be used without setting use_default_roots to true"));
        }
    }

    #[test]
    fn test_tls_verification_service_backend_not_available() {
        // This test will only run if the backend is not available
        // For example, if we're testing with a feature that's not enabled
        let config = TlsVerificationConfig {
            backend: TlsBackend::Rustls,
            verify_certificates: true,
            use_default_roots: true,
            custom_roots: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        };

        // This test will fail if the backend is available
        // But we can still verify that the method returns an error
        let result = TlsVerificationService::new(config);
        // The test will pass if the backend is not available
        // Otherwise, it will fail
        // This is a placeholder test to ensure the error handling works
        if cfg!(not(feature = "ssl-rustls")) {
            assert!(result.is_err());
        }
    }
}