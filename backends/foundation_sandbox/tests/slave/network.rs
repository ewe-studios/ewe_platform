//! The guest network configuration from the kernel command line.

use foundation_sandbox::native::slave::net::{parse, GuestNetwork};

/// The argument that the driver writes becomes the interface configuration.
#[test]
fn parses_the_driver_argument() {
    let cmdline = "console=ttyS0 quiet net.ifnames=0 ewe.net=169.254.169.2/30,169.254.169.1";
    let network = parse(cmdline)
        .expect("the command line parses")
        .expect("a network");
    assert_eq!(
        network,
        GuestNetwork {
            interface: "eth0".to_string(),
            address: "169.254.169.2".parse().expect("address"),
            prefix_len: 30,
            gateway: "169.254.169.1".parse().expect("gateway"),
        }
    );
}

/// A command line without `ewe.net` gives no network and no error.
#[test]
fn absent_network_is_none() {
    let network = parse("console=ttyS0 ewe.listen=vsock:1024").expect("the command line parses");
    assert_eq!(network, None);
}

/// `ewe.iface` names the interface.
#[test]
fn interface_argument_changes_the_name() {
    let cmdline = "ewe.iface=ens3 ewe.net=10.0.0.2/24,10.0.0.1";
    let network = parse(cmdline)
        .expect("the command line parses")
        .expect("a network");
    assert_eq!(network.interface, "ens3");
}

/// A malformed value is an error, not a silent default.
#[test]
fn malformed_network_is_an_error() {
    for cmdline in [
        "ewe.net=10.0.0.2/24",
        "ewe.net=10.0.0.2,10.0.0.1",
        "ewe.net=not-an-address/24,10.0.0.1",
        "ewe.net=10.0.0.2/33,10.0.0.1",
        "ewe.net=10.0.0.2/24,not-a-gateway",
        "ewe.iface=",
    ] {
        assert!(
            parse(cmdline).is_err(),
            "{cmdline} must be rejected, not accepted"
        );
    }
}

/// A `/32` keeps every bit, and a `/0` has no bit.
#[test]
fn prefix_length_is_bounded() {
    let all = parse("ewe.net=10.0.0.2/32,10.0.0.1")
        .expect("parses")
        .expect("a network");
    assert_eq!(all.prefix_len, 32);
    let none = parse("ewe.net=10.0.0.2/0,10.0.0.1")
        .expect("parses")
        .expect("a network");
    assert_eq!(none.prefix_len, 0);
}
