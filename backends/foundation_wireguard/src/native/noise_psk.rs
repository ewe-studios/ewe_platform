//! Symmetric-PSK authenticated byte channel over any transport (spec-55, decision 06).
//!
//! WHY: The bootstrap join channel (and the optional identity-pinned mTLS channel) must be
//! **encrypted and authenticated by a shared secret alone** — no certificates, no PKI, and
//! **no BoringSSL/OpenSSL C library** (which clashes at link time with `libssh2`/OpenSSL in
//! the same binary). A wrong key must simply fail the handshake.
//!
//! WHAT: [`connect`]/[`accept`] wrap any `Read + Write` transport in a [`NoiseStream`] — a
//! Noise-protocol session keyed by a 32-byte pre-shared key. [`NoiseStream`] itself is
//! `Read + Write`, so higher layers (the bootstrap HTTP/JSON framing) run over it unchanged.
//!
//! HOW: `Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s` via the pure-Rust [`snow`] crate. `NN` carries
//! no static keys, so authentication rests entirely on the pre-shared key (mixed in at the
//! start via `psk0`); the ephemeral `ee` DH still provides forward secrecy — an improvement
//! over the CBC-SHA TLS-PSK suites this replaces. After the two-message handshake the session
//! moves to transport mode and each write becomes one length-prefixed, AEAD-sealed frame.

use std::io::{self, Read, Write};

use snow::{Builder, TransportState};

use crate::shared::error::{WgError, WgResult};

/// Noise handshake pattern: NN (no static keys) with the PSK applied first (`psk0`),
/// x25519 DH, ChaCha20-Poly1305 AEAD, BLAKE2s hashing — all pure Rust.
const NOISE_PATTERN: &str = "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s";

/// A Noise transport message is at most 65535 bytes (2-byte length prefix on the wire).
const MAX_NOISE_MSG: usize = 65535;

/// Poly1305 authentication tag length appended to every ciphertext.
const TAG_LEN: usize = 16;

/// Largest plaintext we seal into a single frame (leaving room for the AEAD tag).
const MAX_PLAINTEXT_CHUNK: usize = MAX_NOISE_MSG - TAG_LEN;

/// WHY: The dialing side of a PSK-authenticated session.
///
/// WHAT: Perform the Noise initiator handshake over `inner` keyed by `psk`, returning a
/// transport-mode [`NoiseStream`].
///
/// HOW: Send message 1 (`-> psk, e`), read message 2 (`<- e, ee`), then enter transport mode.
///
/// # Errors
/// [`WgError::Protocol`] if the handshake fails (e.g. the peer holds a different PSK) or the
/// peer closes before completing it; [`WgError::Io`] on transport errors.
pub fn connect<S: Read + Write>(mut inner: S, psk: [u8; 32]) -> WgResult<NoiseStream<S>> {
    let params = NOISE_PATTERN
        .parse()
        .map_err(|e| WgError::Protocol(format!("noise params: {e}")))?;
    let mut handshake = Builder::new(params)
        .psk(0, &psk)
        .map_err(noise_err)?
        .build_initiator()
        .map_err(noise_err)?;

    let mut scratch = vec![0u8; MAX_NOISE_MSG];
    // -> psk, e
    let n = handshake.write_message(&[], &mut scratch).map_err(noise_err)?;
    write_frame(&mut inner, &scratch[..n])?;
    // <- e, ee
    let msg = read_frame(&mut inner)?
        .ok_or_else(|| WgError::Protocol("peer closed during noise handshake".into()))?;
    handshake.read_message(&msg, &mut scratch).map_err(noise_err)?;

    let transport = handshake.into_transport_mode().map_err(noise_err)?;
    Ok(NoiseStream::new(inner, transport))
}

/// WHY: The accepting side of a PSK-authenticated session.
///
/// WHAT: Perform the Noise responder handshake over `inner` keyed by `psk`, returning a
/// transport-mode [`NoiseStream`].
///
/// HOW: Read message 1 (`-> psk, e`), send message 2 (`<- e, ee`), then enter transport mode.
/// A wrong PSK fails when reading message 1 (the AEAD tag will not verify).
///
/// # Errors
/// [`WgError::Protocol`] if the handshake fails or the peer closes before completing it;
/// [`WgError::Io`] on transport errors.
pub fn accept<S: Read + Write>(mut inner: S, psk: [u8; 32]) -> WgResult<NoiseStream<S>> {
    let params = NOISE_PATTERN
        .parse()
        .map_err(|e| WgError::Protocol(format!("noise params: {e}")))?;
    let mut handshake = Builder::new(params)
        .psk(0, &psk)
        .map_err(noise_err)?
        .build_responder()
        .map_err(noise_err)?;

    let mut scratch = vec![0u8; MAX_NOISE_MSG];
    // -> psk, e
    let msg = read_frame(&mut inner)?
        .ok_or_else(|| WgError::Protocol("peer closed during noise handshake".into()))?;
    handshake.read_message(&msg, &mut scratch).map_err(noise_err)?;
    // <- e, ee
    let n = handshake.write_message(&[], &mut scratch).map_err(noise_err)?;
    write_frame(&mut inner, &scratch[..n])?;

    let transport = handshake.into_transport_mode().map_err(noise_err)?;
    Ok(NoiseStream::new(inner, transport))
}

/// A transport-mode Noise session that presents as a plain `Read + Write` byte stream.
///
/// Each [`Write::write`] seals up to [`MAX_PLAINTEXT_CHUNK`] bytes into one length-prefixed
/// AEAD frame; each [`Read::read`] serves bytes from the most recently opened frame, decrypting
/// the next frame on demand. Frames are processed strictly in order (the underlying transport is
/// reliable and ordered), which keeps the Noise nonces in lock-step between the two peers.
pub struct NoiseStream<S: Read + Write> {
    inner: S,
    transport: TransportState,
    /// Decrypted plaintext from the current frame not yet handed to the caller.
    plaintext: Vec<u8>,
    /// Read cursor into `plaintext`.
    offset: usize,
}

impl<S: Read + Write> NoiseStream<S> {
    fn new(inner: S, transport: TransportState) -> Self {
        Self {
            inner,
            transport,
            plaintext: Vec::new(),
            offset: 0,
        }
    }

    /// Borrow the underlying transport (e.g. to read its peer address).
    pub fn get_ref(&self) -> &S {
        &self.inner
    }
}

impl<S: Read + Write> std::fmt::Debug for NoiseStream<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseStream")
            .field("buffered", &(self.plaintext.len() - self.offset))
            .finish_non_exhaustive()
    }
}

impl<S: Read + Write> Read for NoiseStream<S> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.offset >= self.plaintext.len() {
            let frame = match read_frame(&mut self.inner)? {
                // Clean close on a frame boundary is a normal end-of-stream.
                None => return Ok(0),
                Some(frame) => frame,
            };
            let mut plaintext = vec![0u8; frame.len()];
            let n = self
                .transport
                .read_message(&frame, &mut plaintext)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("noise decrypt: {e}")))?;
            plaintext.truncate(n);
            self.plaintext = plaintext;
            self.offset = 0;
        }
        let available = &self.plaintext[self.offset..];
        let take = available.len().min(out.len());
        out[..take].copy_from_slice(&available[..take]);
        self.offset += take;
        Ok(take)
    }
}

impl<S: Read + Write> Write for NoiseStream<S> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let chunk = &data[..data.len().min(MAX_PLAINTEXT_CHUNK)];
        let mut sealed = vec![0u8; chunk.len() + TAG_LEN];
        let n = self
            .transport
            .write_message(chunk, &mut sealed)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("noise encrypt: {e}")))?;
        write_frame(&mut self.inner, &sealed[..n])?;
        Ok(chunk.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Write a single `u16`-length-prefixed frame and flush it.
fn write_frame(inner: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let len = u16::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "noise frame exceeds 65535 bytes"))?;
    inner.write_all(&len.to_be_bytes())?;
    inner.write_all(payload)?;
    inner.flush()
}

/// Read one `u16`-length-prefixed frame. Returns `Ok(None)` on a clean close at a frame
/// boundary (EOF before the first length byte), distinguishing it from a truncated frame.
fn read_frame(inner: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 2];
    if inner.read(&mut len_buf[..1])? == 0 {
        return Ok(None);
    }
    inner.read_exact(&mut len_buf[1..])?;
    let len = usize::from(u16::from_be_bytes(len_buf));
    let mut payload = vec![0u8; len];
    inner.read_exact(&mut payload)?;
    Ok(Some(payload))
}

fn noise_err(err: snow::Error) -> WgError {
    WgError::Protocol(format!("noise handshake failed: {err}"))
}
