use axum::body;
use http::StatusCode;
use http_body_util::BodyExt;
use hyper::client;
use hyper::server;
use hyper::service;
use hyper::upgrade;
use hyper_util::rt;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::{net, sync::broadcast};

use crate::empty;
use crate::full;
use crate::host_addr;
use crate::types::Http1;
use crate::types::Result;
use crate::types::Tunnel;
use crate::StreamError;

const DEFAULT_BUF_SIZE: usize = 1024;

pub async fn stream_writes<R, W>(
    reader: &mut R,
    writer: &mut W,
    mut cancel_signal: broadcast::Receiver<()>,
) -> tokio::io::Result<usize>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut copied = 0;
    let mut buf = [0u8; DEFAULT_BUF_SIZE];

    loop {
        let bytes_read;

        tokio::select! {
            // switch select behaviour to read channels
            // as we define them in order
            biased;

            op = reader.read(&mut buf) => {
                use std::io::ErrorKind::{ConnectionAborted, ConnectionReset};
                bytes_read = op.or_else(|e| match e.kind() {
                    // Consider these types of errors part of life and not actual errors,
                    ConnectionReset | ConnectionAborted => Ok(0),
                    _ => Err(e),
                })?;
            },

            _ = cancel_signal.recv() => {
                break;
            }
        }

        if bytes_read == 0 {
            break;
        }

        match writer.write_all(&buf[0..bytes_read]).await {
            Ok(_) => {
                copied += bytes_read;
            }
            Err(e) => {
                ewe_trace::error!("Failed to write data to destination: {:?}", e)
            }
        }
    }

    Ok(copied)
}

/// Handles bare tcp connection streaming from target source to destination as
/// described by the `ProxyRemoteConfig` for the destination.
pub async fn stream_tunnel(
    mut source: net::TcpStream,
    service_addr: SocketAddr,
    tunnel: Tunnel,
) -> Result<()> {
    let destination_config = tunnel.destination;
    ewe_trace::info!(
        "Starting streaming between client addr {} and destination {} ",
        service_addr,
        destination_config
    );

    let mut remote = match net::TcpStream::connect(destination_config.to_string()).await {
        Ok(r) => r,
        Err(err) => {
            return Err(Box::new(err));
        }
    };

    let (cancel_alert, _cancel_signal) = broadcast::channel::<()>(1);

    let (mut source_reader, mut source_writer) = source.split();
    let (mut destination_reader, mut destination_writer) = remote.split();

    let (source_to_destination, destination_to_source) = tokio::join! {
        stream_writes(&mut source_reader, &mut destination_writer, cancel_alert.subscribe()),
        stream_writes(&mut destination_reader, &mut source_writer, cancel_alert.subscribe()),
    };

    match source_to_destination {
        Ok(copied) => {
            ewe_trace::info!("Copied total bytes: {} from source to destination", copied);
        }
        Err(err) => {
            ewe_trace::error!("Failed in data transmission to destination: {:?}", err);
        }
    };

    match destination_to_source {
        Ok(copied) => {
            ewe_trace::info!("Copied total bytes: {} from destination to source", copied);
        }
        Err(err) => {
            ewe_trace::error!("Failed in data transmission to source: {:?}", err);
        }
    };

    Ok(())
}

/// Handles Http1 proxy streaming for http requests with more customization
/// on the type of request going through.
pub async fn stream_http1(
    source_stream: rt::TokioIo<tokio::net::TcpStream>,
    service_addr: SocketAddr,
    directive: crate::types::Http1,
) -> Result<()> {
    let handler = Http1Service(service_addr, directive);

    match server::conn::http1::Builder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .serve_connection(source_stream, handler)
        .with_upgrades()
        .await
    {
        Ok(_) => {
            ewe_trace::info!("Finished serving http1 request");
            Ok(())
        }
        Err(err) => {
            ewe_trace::error!("Failed to stream http1 connection correctly");
            Err(Box::new(StreamError::FailedStreaming(Box::new(err))).into())
        }
    }
}

/// Http1Service implements the necessary underlying logic
/// to stream a HTTP1 Protocol connection to desired destination.
struct Http1Service(SocketAddr, Http1);

type HttpFuture<R, E> = dyn Future<Output = result::Result<R, E>> + Sync + Send + 'static;

impl service::Service<crate::types::HyperRequest> for Http1Service {
    type Error = hyper::Error;
    type Response = crate::types::HyperResponse;
    type Future = Pin<Box<HttpFuture<Self::Response, Self::Error>>>;

    fn call(&self, req: crate::types::HyperRequest) -> Self::Future {
        let req_path = req.uri().path();
        if let Some(static_routes) = &self.1.routes {
            if let Some(handler) = static_routes.get(req_path) {
                return handler(self.0.clone(), req);
            }
        }

        let proxy_addr = self.1.destination.to_string();
        let stream_operation = async move {
            if req.method() != hyper::Method::CONNECT {
                return match net::TcpStream::connect(proxy_addr.clone()).await {
                    Ok(destination_conn) => {
                        let destination_stream = rt::TokioIo::new(destination_conn);

                        match client::conn::http1::Builder::new()
                            .preserve_header_case(true)
                            .title_case_headers(true)
                            .handshake(destination_stream)
                            .await
                        {
                            Ok((mut request_sender, sender_conn_handle)) => {
                                let _ = tokio::spawn(async move {
                                    if let Err(err) = sender_conn_handle.await {
                                        ewe_trace::error!(
                                            "Connection to destination failed: {} due to error {}",
                                            proxy_addr,
                                            err,
                                        );
                                    }
                                });

                                match request_sender.send_request(req).await {
                                    Ok(destination_response) => {
                                        let resp = destination_response
                                            .map(|b| body::Body::new(b.boxed()));
                                        Ok(resp)
                                    }
                                    Err(err) => {
                                        ewe_trace::error!(
                                            "Request proxy response not received: {:?}",
                                            err
                                        );
                                        Ok(hyper::Response::builder()
                                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                                            .body(body::Body::new(empty()))
                                            .unwrap())
                                    }
                                }
                            }
                            Err(err) => {
                                ewe_trace::error!(
                                    "Failed to build proxy request sender: {:?}",
                                    err
                                );
                                Ok(hyper::Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(body::Body::new(empty()))
                                    .unwrap())
                            }
                        }
                    }
                    Err(err) => {
                        ewe_trace::error!(
                            "Failed to connect to proxy destination {} due to: {:?}",
                            proxy_addr,
                            err
                        );
                        Ok(hyper::Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(body::Body::new(empty()))
                            .unwrap())
                    }
                };
            }

            // Received an HTTP request like:
            // ```
            // CONNECT www.domain.com:443 HTTP/1.1
            // Host: www.domain.com:443
            // Proxy-Connection: Keep-Alive
            // ```
            //
            // When HTTP method is CONNECT we should return an empty body
            // then we can eventually upgrade the connection and talk a new protocol.
            //
            // Note: only after client received an empty body with STATUS_OK can the
            // connection be upgraded, so we can't return a response inside
            // `on_upgrade` future.
            match host_addr(req.uri()) {
                Some(socket_addr) => match hyper::upgrade::on(req).await {
                    Ok(upgraded) => {
                        let _ = tokio::task::spawn(async move {
                            if let Err(failed_err) = stream_http_bidrectional(
                                String::from("HTTP1"),
                                upgraded,
                                socket_addr.clone(),
                            )
                            .await
                            {
                                ewe_trace::error!("Failed to stream bi-directional CONNECT request to: {} due to {}", socket_addr, failed_err)
                            }
                        });
                        Ok(hyper::Response::new(body::Body::new(empty())))
                    }
                    Err(err) => {
                        ewe_trace::error!(
                            "Failed to upgrade connection for socket addr: {} due to {}",
                            socket_addr,
                            err
                        );
                        Ok(hyper::Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(body::Body::new(full(
                                "Failed to successfully upgrade connection!",
                            )))
                            .unwrap())
                    }
                },
                None => Ok(hyper::Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(body::Body::new(full(
                        "CONNECT must alwayas come with a socket address!",
                    )))
                    .unwrap()),
            }
        };

        Box::pin(stream_operation)
    }
}

// Create a TCP connection to host:port, build a tunnel between the connection and
// the upgraded connection
async fn stream_http_bidrectional(
    protocol: String,
    upgraded: upgrade::Upgraded,
    addr: String,
) -> std::io::Result<()> {
    // Connect to remote server
    let mut server = net::TcpStream::connect(addr).await?;
    let mut upgraded = rt::TokioIo::new(upgraded);

    // Proxying data
    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    // Print message when done
    ewe_trace::info!(
        "Finished {} client wrote {} bytes and received {} bytes",
        protocol,
        from_client,
        from_server
    );

    Ok(())
}
