// TunnelProxy — handles non-HTTP traffic on the same port.
// Copies bidirectionally to upstream via raw TCP.

use std::io::{Read, Write};
use std::net::TcpStream;

use foundation_http::shared::serve::ConnectionResult;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::HTTPStreams;

/// Copies data bidirectionally between two streams.
pub fn copy_bidirectional<R: Read + Write, S: Read + Write>(a: &mut R, b: &mut S) {
    let mut buf_a = [0u8; 8192];
    let mut buf_b = [0u8; 8192];

    loop {
        match a.read(&mut buf_a) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let _ = b.write_all(&buf_a[..n]);
            }
        }
        match b.read(&mut buf_b) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let _ = a.write_all(&buf_b[..n]);
            }
        }
    }
}

// -- TunnelProxyTrait (for foundation_http extension)

/// User-provided handler for non-HTTP connections on the same port.
pub trait TunnelProxyTrait: Send + Clone + 'static {
    fn handle(
        &self,
        conn: SharedByteBufferStream<RawStream>,
        streams: HTTPStreams<RawStream>,
        client_ip: &str,
    ) -> ConnectionResult;
}

// -- TunnelProxy impl

#[derive(Clone)]
pub struct TunnelProxy {
    pub dest: String,
}

impl TunnelProxyTrait for TunnelProxy {
    fn handle(
        &self,
        mut conn: SharedByteBufferStream<RawStream>,
        _streams: HTTPStreams<RawStream>,
        _client_ip: &str,
    ) -> ConnectionResult {
        match TcpStream::connect(&self.dest) {
            Ok(mut upstream) => {
                copy_bidirectional(&mut conn, &mut upstream);
                ConnectionResult::Take
            }
            Err(e) => {
                tracing::error!("Tunnel upstream connect failed: {e}");
                ConnectionResult::Close(None)
            }
        }
    }
}
