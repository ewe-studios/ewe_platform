//! Optional identity-pinned mutual TLS over the overlay (spec-55, feature 08; decision 10).
//!
//! WHY: WireGuard's Noise handshake already gives mutual auth + forward secrecy, so mTLS
//! is **off by default**. When a service wants app-layer mutual auth pinned to the WG
//! identity keys — with **no external CA** — this provides it.
//!
//! WHAT: [`connect`]/[`accept`] wrap any byte stream in a TLS-PSK session whose PSK is the
//! x25519-ECDH secret between the two identity keys ([`IdentityKeypair::derive_shared_psk`]).
//! Only the holders of the two identity secrets can derive the PSK, so a successful
//! handshake authenticates the peer *as* that identity key.
//!
//! HOW: BoringSSL TLS 1.2 PSK (same suites as the bootstrap channel), keyed by the derived
//! PSK. Generic over the transport, so it wraps an `OverlayStream` (via a blocking adapter)
//! or any `Read + Write`.

use std::io::{Read, Write};

use boring::ssl::{Ssl, SslContext, SslContextBuilder, SslMethod, SslStream, SslVersion};

use crate::shared::error::{WgError, WgResult};

/// PSK cipher suites (TLS 1.2) — BoringSSL ships no GCM PSK, so CBC-SHA (see F02).
const PSK_CIPHERS: &str = "PSK-AES256-CBC-SHA:PSK-AES128-CBC-SHA";
/// Fixed PSK identity hint for the mTLS channel (the real binding is the derived PSK).
const MTLS_IDENTITY: &[u8] = b"fwg-mtls";

fn context(server: bool, psk: [u8; 32]) -> WgResult<SslContext> {
    let mut builder =
        SslContextBuilder::new(SslMethod::tls()).map_err(|e| err("context", &e))?;
    builder
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .map_err(|e| err("min version", &e))?;
    builder
        .set_max_proto_version(Some(SslVersion::TLS1_2))
        .map_err(|e| err("max version", &e))?;
    builder
        .set_cipher_list(PSK_CIPHERS)
        .map_err(|e| err("cipher list", &e))?;
    if server {
        builder.set_psk_server_callback(move |_ssl, _identity, psk_out| {
            let n = psk.len().min(psk_out.len());
            psk_out[..n].copy_from_slice(&psk[..n]);
            Ok(n)
        });
    } else {
        builder.set_psk_client_callback(move |_ssl, _hint, identity_out, psk_out| {
            if MTLS_IDENTITY.len() + 1 > identity_out.len() {
                return Ok(0);
            }
            identity_out[..MTLS_IDENTITY.len()].copy_from_slice(MTLS_IDENTITY);
            identity_out[MTLS_IDENTITY.len()] = 0;
            let n = psk.len().min(psk_out.len());
            psk_out[..n].copy_from_slice(&psk[..n]);
            Ok(n)
        });
    }
    Ok(builder.build())
}

/// WHY: The dialing side of an identity-pinned mTLS session.
///
/// WHAT: Wrap `stream` as a TLS-PSK client keyed by `psk` (the derived shared secret).
///
/// HOW: BoringSSL PSK client handshake over the transport.
///
/// # Errors
/// [`WgError::Protocol`] if the handshake fails (e.g. the peer derived a different PSK,
/// i.e. it is not the expected identity).
pub fn connect<S: Read + Write>(stream: S, psk: [u8; 32]) -> WgResult<SslStream<S>> {
    let ctx = context(false, psk)?;
    let ssl = Ssl::new(&ctx).map_err(|e| err("ssl", &e))?;
    let mut tls = SslStream::new(ssl, stream).map_err(|e| err("ssl stream", &e))?;
    tls.connect()
        .map_err(|e| WgError::Protocol(format!("mtls handshake failed: {e}")))?;
    Ok(tls)
}

/// WHY: The accepting side of an identity-pinned mTLS session.
///
/// WHAT: Wrap `stream` as a TLS-PSK server keyed by `psk`.
///
/// HOW: BoringSSL PSK server handshake over the transport.
///
/// # Errors
/// [`WgError::Protocol`] if the handshake fails.
pub fn accept<S: Read + Write>(stream: S, psk: [u8; 32]) -> WgResult<SslStream<S>> {
    let ctx = context(true, psk)?;
    let ssl = Ssl::new(&ctx).map_err(|e| err("ssl", &e))?;
    let mut tls = SslStream::new(ssl, stream).map_err(|e| err("ssl stream", &e))?;
    tls.accept()
        .map_err(|e| WgError::Protocol(format!("mtls handshake failed: {e}")))?;
    Ok(tls)
}

fn err(what: &str, e: &boring::error::ErrorStack) -> WgError {
    WgError::Protocol(format!("mtls {what}: {e}"))
}
