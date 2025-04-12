#[derive(Clone, Debug)]
pub struct DataStreamAddr(core::net::SocketAddr, core::net::SocketAddr);

// --- Constructors

impl DataStreamAddr {
    pub fn new(local_addr: core::net::SocketAddr, remote_addr: core::net::SocketAddr) -> Self {
        Self(local_addr, remote_addr)
    }
}

// --- Methods

impl DataStreamAddr {
    #[inline]
    pub fn peer_addr(&self) -> core::net::SocketAddr {
        self.1
    }

    #[inline]
    pub fn local_addr(&self) -> core::net::SocketAddr {
        self.0
    }
}

/// DataStream defines a trait defining an expected
/// network stream of data of type `NetworkStreams::BodyType`.
pub trait DataStream {
    type Error;
    type Headers;
    type BodyType;

    /// headers returns a `Option<HashMap<String, String>>` which might
    /// contains the related headers of the underlying streams.
    fn headers(&self) -> std::result::Result<Self::Headers, Self::Error>;

    /// body returns an iterator that returns the underlying type
    /// the `NakedStream` represents.
    fn body<S>(&self) -> S
    where
        S: Iterator<Item = Self::BodyType>;
}

/// IntoHeaders defines a trait that allows us customize the
/// transformation of a byte slice into a header of a selected type
pub trait IntoHeaders {
    type Error;
    type Headers;

    /// into_headers returns the Header representation for a giving byteslice reference
    /// which allows us customize how headers are really generated.
    fn into_headers(content: &[u8]) -> std::result::Result<Self::Headers, Self::Error>;
}
