# Smol Examples

```rust
//! A TCP chat client.
//!
//! First start a server:
//!
//! ``
//! cargo run --example chat-server
//! ``
//!
//! Then start clients:
//!
//! ``
//! cargo run --example chat-client
//! ``

use std::net::TcpStream;

use smol::{future, io, Async, Unblock};

fn main() -> io::Result<()> {
    smol::block_on(async {
        // Connect to the server and create async stdin and stdout.
        let stream = Async::<TcpStream>::connect(([127, 0, 0, 1], 6000)).await?;
        let stdin = Unblock::new(std::io::stdin());
        let mut stdout = Unblock::new(std::io::stdout());

        // Intro messages.
        println!("Connected to {}", stream.get_ref().peer_addr()?);
        println!("My nickname: {}", stream.get_ref().local_addr()?);
        println!("Type a message and hit enter!\n");

        let reader = &stream;
        let mut writer = &stream;

        // Wait until the standard input is closed or the connection is closed.
        future::race(
            async {
                let res = io::copy(stdin, &mut writer).await;
                println!("Quit!");
                res
            },
            async {
                let res = io::copy(reader, &mut stdout).await;
                println!("Server disconnected!");
                res
            },
        )
        .await?;

        Ok(())
    })
}

```

```rust
//! Connect to an HTTP website, make a GET request, and pipe the response to the standard output.
//!
//! Run with:
//!
//! ``
//! cargo run --example get-request
//! ``

use smol::{io, prelude::*, Async, Unblock};
use std::net::{TcpStream, ToSocketAddrs};

fn main() -> io::Result<()> {
    smol::block_on(async {
        // Connect to http://example.com
        let mut addrs = smol::unblock(move || ("example.com", 80).to_socket_addrs()).await?;
        let addr = addrs.next().unwrap();
        let mut stream = Async::<TcpStream>::connect(addr).await?;

        // Send an HTTP GET request.
        let req = b"GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n";
        stream.write_all(req).await?;

        // Read the response and pipe it to the standard output.
        let mut stdout = Unblock::new(std::io::stdout());
        io::copy(&stream, &mut stdout).await?;
        Ok(())
    })
}


```

```rust

//! An HTTP+TLS server based on `hyper` and `async-native-tls`.
//!
//! Run with:
//!
//! ``
//! cargo run --example hyper-server
//! ``
//!
//! Open in the browser any of these addresses:
//!
//! - http://localhost:8000/
//! - https://localhost:8001/ (accept the security prompt in the browser)
//!
//! Refer to `README.md` to see how to the TLS certificate was generated.

use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Result;
use async_native_tls::{Identity, TlsAcceptor, TlsStream};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response};
use macro_rules_attribute::apply;
use smol::{future, io, prelude::*, Async, Executor};
use smol_hyper::rt::{FuturesIo, SmolTimer};
use smol_macros::main;

/// Serves a request and returns a response.
async fn serve(req: Request<Incoming>) -> Result<Response<Full<&'static [u8]>>> {
    println!("Serving {}", req.uri());
    Ok(Response::new(Full::new("Hello from hyper!".as_bytes())))
}

/// Handle a new client.
async fn handle_client(client: Async<TcpStream>, tls: Option<TlsAcceptor>) -> Result<()> {
    // Wrap it in TLS if necessary.
    let client = match &tls {
        None => SmolStream::Plain(client),
        Some(tls) => {
            // In case of HTTPS, establish a secure TLS connection.
            SmolStream::Tls(tls.accept(client).await?)
        }
    };

    // Build the server.
    hyper::server::conn::http1::Builder::new()
        .timer(SmolTimer::new())
        .serve_connection(FuturesIo::new(client), service_fn(serve))
        .await?;

    Ok(())
}

/// Listens for incoming connections and serves them.
async fn listen(
    ex: &Arc<Executor<'static>>,
    listener: Async<TcpListener>,
    tls: Option<TlsAcceptor>,
) -> Result<()> {
    // Format the full host address.
    let host = &match tls {
        None => format!("http://{}", listener.get_ref().local_addr()?),
        Some(_) => format!("https://{}", listener.get_ref().local_addr()?),
    };
    println!("Listening on {}", host);

    loop {
        // Wait for a new client.
        let (client, _) = listener.accept().await?;

        // Spawn a task to handle this connection.
        ex.spawn({
            let tls = tls.clone();
            async move {
                if let Err(e) = handle_client(client, tls).await {
                    println!("Error while handling client: {}", e);
                }
            }
        })
        .detach();
    }
}

#[apply(main!)]
async fn main(ex: &Arc<Executor<'static>>) -> Result<()> {
    // Initialize TLS with the local certificate, private key, and password.
    let identity = Identity::from_pkcs12(include_bytes!("identity.pfx"), "password")?;
    let tls = TlsAcceptor::from(native_tls::TlsAcceptor::new(identity)?);

    // Start HTTP and HTTPS servers.
    let http = listen(
        ex,
        Async::<TcpListener>::bind(([127, 0, 0, 1], 8000))?,
        None,
    );
    let https = listen(
        ex,
        Async::<TcpListener>::bind(([127, 0, 0, 1], 8001))?,
        Some(tls),
    );
    future::try_zip(http, https).await?;
    Ok(())
}

/// A TCP or TCP+TLS connection.
enum SmolStream {
    /// A plain TCP connection.
    Plain(Async<TcpStream>),

    /// A TCP connection secured by TLS.
    Tls(TlsStream<Async<TcpStream>>),
}

impl AsyncRead for SmolStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Self::Plain(s) => Pin::new(s).poll_read(cx, buf),
            Self::Tls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for SmolStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Self::Plain(s) => Pin::new(s).poll_write(cx, buf),
            Self::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Plain(s) => Pin::new(s).poll_close(cx),
            Self::Tls(s) => Pin::new(s).poll_close(cx),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Plain(s) => Pin::new(s).poll_close(cx),
            Self::Tls(s) => Pin::new(s).poll_close(cx),
        }
    }
}

```


```rust

//! A simple HTTP+TLS client based on `async-native-tls`.
//!
//! Run with:
//!
//! ``
//! cargo run --example simple-client
//! ``

use std::net::{TcpStream, ToSocketAddrs};

use anyhow::{bail, Context as _, Result};
use smol::{prelude::*, Async};
use url::Url;

/// Sends a GET request and fetches the response.
async fn fetch(addr: &str) -> Result<Vec<u8>> {
    // Parse the URL.
    let url = Url::parse(addr)?;
    let host = url.host().context("cannot parse host")?.to_string();
    let port = url.port_or_known_default().context("cannot guess port")?;
    let path = url.path().to_string();
    let query = match url.query() {
        Some(q) => format!("?{}", q),
        None => String::new(),
    };

    // Construct a request.
    let req = format!(
        "GET {}{} HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        path, query, host,
    );

    // Connect to the host.
    let socket_addr = {
        let host = host.clone();
        smol::unblock(move || (host.as_str(), port).to_socket_addrs())
            .await?
            .next()
            .context("cannot resolve address")?
    };
    let mut stream = Async::<TcpStream>::connect(socket_addr).await?;

    // Send the request and wait for the response.
    let mut resp = Vec::new();
    match url.scheme() {
        "http" => {
            stream.write_all(req.as_bytes()).await?;
            stream.read_to_end(&mut resp).await?;
        }
        "https" => {
            // In case of HTTPS, establish a secure TLS connection first.
            let mut stream = async_native_tls::connect(&host, stream).await?;
            stream.write_all(req.as_bytes()).await?;
            stream.read_to_end(&mut resp).await?;
        }
        scheme => bail!("unsupported scheme: {}", scheme),
    }

    Ok(resp)
}

fn main() -> Result<()> {
    smol::block_on(async {
        let addr = "https://www.rust-lang.org";
        let resp = fetch(addr).await?;
        println!("{}", String::from_utf8_lossy(&resp));
        Ok(())
    })
}

```


```rust

//! Crawls the Rust language website and prints found pages.
//!
//! Run with:
//!
//! ``
//! cargo run --example web-crawler
//! ``

use std::collections::{HashSet, VecDeque};

use anyhow::Result;
use async_channel::{bounded, Sender};
use scraper::{Html, Selector};

const ROOT: &str = "https://www.rust-lang.org";

/// Fetches the HTML contents of a web page.
async fn fetch(url: String, sender: Sender<String>) {
    let body = surf::get(&url).recv_string().await;
    let body = body.unwrap_or_default();
    sender.send(body).await.ok();
}

/// Extracts links from a HTML body.
fn links(body: String) -> Vec<String> {
    let mut v = Vec::new();
    for elem in Html::parse_fragment(&body).select(&Selector::parse("a").unwrap()) {
        if let Some(link) = elem.value().attr("href") {
            v.push(link.to_string());
        }
    }
    v
}

fn main() -> Result<()> {
    smol::block_on(async {
        let mut seen = HashSet::new();
        let mut queue = VecDeque::new();
        seen.insert(ROOT.to_string());
        queue.push_back(ROOT.to_string());

        let (s, r) = bounded(200);
        let mut tasks = 0;

        // Loop while the queue is not empty or tasks are fetching pages.
        while queue.len() + tasks > 0 {
            // Limit the number of concurrent tasks.
            while tasks < s.capacity().unwrap() {
                // Process URLs in the queue and fetch more pages.
                match queue.pop_front() {
                    None => break,
                    Some(url) => {
                        println!("{}", url);
                        tasks += 1;
                        smol::spawn(fetch(url, s.clone())).detach();
                    }
                }
            }

            // Get a fetched web page.
            let body = r.recv().await.unwrap();
            tasks -= 1;

            // Parse links in the web page and add them to the queue.
            for mut url in links(body) {
                // Add the site prefix if it's missing.
                if url.starts_with('/') {
                    url = format!("{}{}", ROOT, url);
                }

                // If the URL makes sense and was not seen already, push it into the queue.
                if url.starts_with(ROOT) && seen.insert(url.clone()) {
                    url = url.trim_end_matches('/').to_string();
                    queue.push_back(url);
                }
            }
        }
        Ok(())
    })
}

```


```rust
// Server
//! A WebSocket+TLS echo server based on `async-tungstenite` and `async-native-tls`.
//!
//! First start a server:
//!
//! ``
//! cargo run --example websocket-server
//! ``
//!
//! Then start a client:
//!
//! ``
//! cargo run --example websocket-client
//! ``

use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{Context as _, Result};
use async_native_tls::{Identity, TlsAcceptor, TlsStream};
use async_tungstenite::{tungstenite, WebSocketStream};
use futures::sink::{Sink, SinkExt};
use smol::{future, prelude::*, Async};
use tungstenite::Message;

/// Echoes messages from the client back to it.
async fn echo(mut stream: WsStream) -> Result<()> {
    let msg = stream.next().await.context("expected a message")??;
    stream.send(Message::text(msg.to_string())).await?;
    Ok(())
}

/// Listens for incoming connections and serves them.
async fn listen(listener: Async<TcpListener>, tls: Option<TlsAcceptor>) -> Result<()> {
    let host = match &tls {
        None => format!("ws://{}", listener.get_ref().local_addr()?),
        Some(_) => format!("wss://{}", listener.get_ref().local_addr()?),
    };
    println!("Listening on {}", host);

    loop {
        // Accept the next connection.
        let (stream, _) = listener.accept().await?;
        println!("Accepted client: {}", stream.get_ref().peer_addr()?);

        match &tls {
            None => {
                let stream = WsStream::Plain(async_tungstenite::accept_async(stream).await?);
                smol::spawn(echo(stream)).detach();
            }
            Some(tls) => {
                // In case of WSS, establish a secure TLS connection first.
                let stream = tls.accept(stream).await?;
                let stream = WsStream::Tls(async_tungstenite::accept_async(stream).await?);
                smol::spawn(echo(stream)).detach();
            }
        }
    }
}

fn main() -> Result<()> {
    // Initialize TLS with the local certificate, private key, and password.
    let identity = Identity::from_pkcs12(include_bytes!("identity.pfx"), "password")?;
    let tls = TlsAcceptor::from(native_tls::TlsAcceptor::new(identity)?);

    // Start WS and WSS servers.
    smol::block_on(async {
        let ws = listen(Async::<TcpListener>::bind(([127, 0, 0, 1], 9000))?, None);
        let wss = listen(
            Async::<TcpListener>::bind(([127, 0, 0, 1], 9001))?,
            Some(tls),
        );
        future::try_zip(ws, wss).await?;
        Ok(())
    })
}

/// A WebSocket or WebSocket+TLS connection.
enum WsStream {
    /// A plain WebSocket connection.
    Plain(WebSocketStream<Async<TcpStream>>),

    /// A WebSocket connection secured by TLS.
    Tls(WebSocketStream<TlsStream<Async<TcpStream>>>),
}

impl Sink<Message> for WsStream {
    type Error = tungstenite::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).poll_ready(cx),
            WsStream::Tls(s) => Pin::new(s).poll_ready(cx),
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).start_send(item),
            WsStream::Tls(s) => Pin::new(s).start_send(item),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).poll_flush(cx),
            WsStream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).poll_close(cx),
            WsStream::Tls(s) => Pin::new(s).poll_close(cx),
        }
    }
}

impl Stream for WsStream {
    type Item = tungstenite::Result<Message>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).poll_next(cx),
            WsStream::Tls(s) => Pin::new(s).poll_next(cx),
        }
    }
}

```

```rust
// client
//! A WebSocket+TLS client based on `async-tungstenite` and `async-native-tls`.
//!
//! First start a server:
//!
//! ``
//! cargo run --example websocket-server
//! ``
//!
//! Then start a client:
//!
//! ``
//! cargo run --example websocket-client
//! ``

use std::net::{TcpStream, ToSocketAddrs};
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{bail, Context as _, Result};
use async_native_tls::{Certificate, TlsConnector, TlsStream};
use async_tungstenite::{tungstenite, WebSocketStream};
use futures::sink::{Sink, SinkExt};
use smol::{prelude::*, Async};
use tungstenite::handshake::client::Response;
use tungstenite::Message;
use url::Url;

/// Connects to a WebSocket address (optionally secured by TLS).
async fn connect(addr: &str, tls: TlsConnector) -> Result<(WsStream, Response)> {
    // Parse the address.
    let url = Url::parse(addr)?;
    let host = url.host_str().context("cannot parse host")?.to_string();
    let port = url.port_or_known_default().context("cannot guess port")?;

    // Resolve the address.
    let socket_addr = {
        let host = host.clone();
        smol::unblock(move || (host.as_str(), port).to_socket_addrs())
            .await?
            .next()
            .context("cannot resolve address")?
    };

    // Connect to the address.
    match url.scheme() {
        "ws" => {
            let stream = Async::<TcpStream>::connect(socket_addr).await?;
            let (stream, resp) = async_tungstenite::client_async(addr, stream).await?;
            Ok((WsStream::Plain(stream), resp))
        }
        "wss" => {
            // In case of WSS, establish a secure TLS connection first.
            let stream = Async::<TcpStream>::connect(socket_addr).await?;
            let stream = tls.connect(host, stream).await?;
            let (stream, resp) = async_tungstenite::client_async(addr, stream).await?;
            Ok((WsStream::Tls(stream), resp))
        }
        scheme => bail!("unsupported scheme: {}", scheme),
    }
}

fn main() -> Result<()> {
    // Initialize TLS with the local certificate.
    let mut builder = native_tls::TlsConnector::builder();
    builder.add_root_certificate(Certificate::from_pem(include_bytes!("certificate.pem"))?);
    let tls = TlsConnector::from(builder);

    smol::block_on(async {
        // Connect to the server.
        let (mut stream, resp) = connect("wss://127.0.0.1:9001", tls).await?;
        dbg!(resp);

        // Send a message and receive a response.
        stream.send(Message::text("Hello!")).await?;
        dbg!(stream.next().await);

        Ok(())
    })
}

/// A WebSocket or WebSocket+TLS connection.
enum WsStream {
    /// A plain WebSocket connection.
    Plain(WebSocketStream<Async<TcpStream>>),

    /// A WebSocket connection secured by TLS.
    Tls(WebSocketStream<TlsStream<Async<TcpStream>>>),
}

impl Sink<Message> for WsStream {
    type Error = tungstenite::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).poll_ready(cx),
            WsStream::Tls(s) => Pin::new(s).poll_ready(cx),
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).start_send(item),
            WsStream::Tls(s) => Pin::new(s).start_send(item),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).poll_flush(cx),
            WsStream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).poll_close(cx),
            WsStream::Tls(s) => Pin::new(s).poll_close(cx),
        }
    }
}

impl Stream for WsStream {
    type Item = tungstenite::Result<Message>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut *self {
            WsStream::Plain(s) => Pin::new(s).poll_next(cx),
            WsStream::Tls(s) => Pin::new(s).poll_next(cx),
        }
    }
}


```
