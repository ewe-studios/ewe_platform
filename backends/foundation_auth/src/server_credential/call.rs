//! The facts that a client-authentication provider may read for one call.

use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::{Extensions, SimpleHeaders};

/// What a provider may look at for one call.
///
/// It borrows, so it is `Copy` and adds no allocation to the request path.
#[derive(Clone, Copy)]
pub struct CallCredentials<'a> {
    /// The full procedure path, for example
    /// `"/ewe.sandbox.control.v1.SandboxService/Create"`.
    pub procedure: &'a str,
    /// The request headers.
    pub headers: &'a SimpleHeaders,
    /// Peer address and TLS facts. The peer certificate is here on a native
    /// mutual-TLS (mTLS) host.
    pub connection: &'a ConnectionContext,
    /// Facts that the host added, for example `CfTlsClientAuth` on a Worker.
    pub extensions: &'a Extensions,
}
