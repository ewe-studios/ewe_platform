//! The command-line structures of the two programs (feature 06).
//!
//! [`sandbox`] holds `agent-sandbox`; [`slave`] holds `agent-slave`. The shared
//! helpers parse the `--listen` and `--connect` forms.

use std::net::ToSocketAddrs as _;

use crate::shared::listen::ListenAddress;

#[cfg(feature = "control")]
pub mod sandbox;

#[cfg(all(feature = "slave", target_os = "linux"))]
pub mod slave;

/// Parses a `--listen` value: `vsock:<port>`, `unix:<path>` or
/// `tcp:<host>:<port>`.
///
/// # Errors
/// The reason, as text, when the value does not have one of these forms.
pub fn parse_listen(value: &str) -> Result<ListenAddress, String> {
    if let Some(rest) = value.strip_prefix("unix:") {
        if rest.is_empty() {
            return Err("the Unix domain socket path is empty".to_string());
        }
        return Ok(ListenAddress::Unix { path: rest.into() });
    }
    if let Some(rest) = value.strip_prefix("vsock:") {
        let port = rest
            .parse::<u32>()
            .map_err(|_| format!("the vsock port is not a number: {rest}"))?;
        return Ok(ListenAddress::Vsock { port });
    }
    if let Some(rest) = value.strip_prefix("tcp:") {
        let (host, port) = split_host_port(rest)?;
        let mut addresses = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|error| format!("the TCP address {host}:{port} does not resolve: {error}"))?;
        let addr = addresses
            .next()
            .ok_or_else(|| format!("the TCP address {host}:{port} resolved to no address"))?;
        return Ok(ListenAddress::Tcp { addr });
    }
    Err(format!(
        "the listen address must be vsock:<port>, unix:<path> or tcp:<host>:<port>: {value}"
    ))
}

/// Parses a `--connect` value: `unix:<path>`, `tcp:<host>:<port>`, or an
/// `http://` / `https://` base address.
///
/// # Errors
/// The reason, as text, when the value does not have one of these forms.
#[cfg(feature = "control")]
pub fn parse_connect(value: &str) -> Result<crate::native::connect::ConnectAddr, String> {
    use crate::native::connect::ConnectAddr;
    if let Some(rest) = value.strip_prefix("unix:") {
        if rest.is_empty() {
            return Err("the Unix domain socket path is empty".to_string());
        }
        return Ok(ConnectAddr::Unix(rest.into()));
    }
    if let Some(rest) = value.strip_prefix("tcp:") {
        let (host, port) = split_host_port(rest)?;
        return Ok(ConnectAddr::Tcp { host, port });
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return Ok(ConnectAddr::Url(value.to_string()));
    }
    Err(format!(
        "the connect address must be unix:<path>, tcp:<host>:<port> or http(s)://<address>: {value}"
    ))
}

/// Splits `host:port` at the last colon, so an IPv6 address in brackets works.
fn split_host_port(value: &str) -> Result<(String, u16), String> {
    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| format!("the address has no port: {value}"))?;
    if host.is_empty() {
        return Err(format!("the address has no host: {value}"));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| format!("the port is not a number: {port}"))?;
    Ok((host.trim_matches(['[', ']']).to_string(), port))
}
