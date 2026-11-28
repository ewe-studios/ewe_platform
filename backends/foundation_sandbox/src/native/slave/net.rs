//! The guest network configuration of the kernel command line.
//!
//! WHY: A Firecracker guest gets a virtio-net interface, and the host stack gives it a
//! static address on a private link. The guest kernel does not configure the interface
//! by itself, so the slave (process identifier 1) applies the address before it serves.
//!
//! WHAT: [`GuestNetwork`] holds the interface name, the address, the prefix length and
//! the gateway. [`parse`] reads them from the `ewe.net` and `ewe.iface` arguments.
//!
//! HOW: The driver adds `ewe.net=<address>/<prefix>,<gateway>` to the kernel command
//! line. The slave reads `/proc/cmdline` and applies the values through `ioctl` calls.

use std::io;
use std::net::Ipv4Addr;

/// The kernel argument that names the guest address, prefix and gateway.
pub const NET_ARGUMENT: &str = "ewe.net=";

/// The kernel argument that names the guest interface. The default is `eth0`.
pub const IFACE_ARGUMENT: &str = "ewe.iface=";

/// The guest interface configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestNetwork {
    /// The guest interface name, such as `eth0`.
    pub interface: String,
    /// The guest address on the private link.
    pub address: Ipv4Addr,
    /// The prefix length of the link.
    pub prefix_len: u8,
    /// The gateway, which is the host stack address.
    pub gateway: Ipv4Addr,
}

/// Parse `ewe.net` and `ewe.iface` from one kernel command line.
///
/// The form is `ewe.net=<address>/<prefix>,<gateway>`, for example
/// `ewe.net=169.254.169.2/30,169.254.169.1`. `ewe.iface` names the interface and
/// defaults to `eth0`.
///
/// # Errors
/// [`io::ErrorKind::InvalidInput`] when a value is present but malformed.
pub fn parse(cmdline: &str) -> io::Result<Option<GuestNetwork>> {
    let mut network: Option<GuestNetwork> = None;
    let mut interface = "eth0".to_string();
    for word in cmdline.split_whitespace() {
        if let Some(value) = word.strip_prefix(IFACE_ARGUMENT) {
            if value.is_empty() {
                return Err(invalid("ewe.iface is empty"));
            }
            interface = value.to_string();
        }
        let Some(value) = word.strip_prefix(NET_ARGUMENT) else {
            continue;
        };
        let (address, gateway) = value
            .split_once(',')
            .ok_or_else(|| invalid("ewe.net needs <address>/<prefix>,<gateway>"))?;
        let (address, prefix) = address
            .split_once('/')
            .ok_or_else(|| invalid("ewe.net needs the prefix after the address"))?;
        let address = address
            .parse::<Ipv4Addr>()
            .map_err(|_| invalid("ewe.net has an invalid address"))?;
        let prefix_len = prefix
            .parse::<u8>()
            .map_err(|_| invalid("ewe.net has an invalid prefix length"))?;
        if prefix_len > 32 {
            return Err(invalid("ewe.net has a prefix length above 32"));
        }
        let gateway = gateway
            .parse::<Ipv4Addr>()
            .map_err(|_| invalid("ewe.net has an invalid gateway"))?;
        network = Some(GuestNetwork {
            interface: String::new(),
            address,
            prefix_len,
            gateway,
        });
    }
    Ok(network.map(|network| GuestNetwork {
        interface,
        ..network
    }))
}

/// An invalid-argument error.
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
