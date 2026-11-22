//! Errors of the certificate authority (CA) module.

/// Why a certificate operation failed.
///
/// No variant holds a key, a private value or a certificate body. The message
/// names the failed step only.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum PkiError {
    /// A certificate signing request (CSR) is malformed or has a bad signature.
    #[display("invalid certificate signing request: {_0}")]
    BadCsr(#[error(not(source))] String),
    /// A client name is not usable as a certificate subject.
    #[display("invalid client name: {_0}")]
    BadClientName(#[error(not(source))] String),
    /// The CA, a certificate or a signature operation failed.
    #[display("certificate authority error: {_0}")]
    Ca(#[error(not(source))] String),
    /// The CA store could not be read or written.
    #[display("certificate authority store error: {_0}")]
    Store(#[error(not(source))] String),
    /// A key could not be generated, parsed or used.
    #[display("key error: {_0}")]
    Key(#[error(not(source))] String),
}
