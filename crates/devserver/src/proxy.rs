use derive_more::From;
use hyper_util::rt;
use std::net::SocketAddr;
use std::{sync, time};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::oneshot;

use tokio::net;

use crate::streams;
use crate::types::{Http1, Http2, Http3, JoinHandle, Result, Tunnel};
use crate::Operator;

// -- Errors

#[derive(Debug, From)]
pub enum ProxyError {
    FailedProxyConnection,
    ConnectionDrop,
    StreamingFailed,
    TunnelNotSupported(ProxyType),
}

impl std::error::Error for ProxyError {}

impl core::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// -- Proxy Types

#[derive(Debug, Clone)]
pub enum ProxyType {
    Tunnel(Tunnel),
    Http1(Http1),
    Http2(Http2),
    Http3(Http3),
}

impl core::fmt::Display for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl ProxyType {
    async fn tunnel_connection(self, connection: (TcpStream, SocketAddr)) -> Result<()> {
        match self {
            ProxyType::Tunnel(t) => {
                let (client, client_addr) = connection;
                streams::stream_tunnel(client, client_addr.clone(), t.clone()).await?;
                ewe_logs::info!(
                    "Finished serving::tunnel client: {} from {} to {}",
                    client_addr.clone(),
                    t.source,
                    t.destination,
                );
                Ok(())
            }
            _ => Err(Box::new(ProxyError::TunnelNotSupported(self)).into()),
        }
    }

    async fn stream_http1(self, connection: (TcpStream, SocketAddr)) -> Result<()> {
        match self {
            ProxyType::Http1(t) => {
                let (client, client_addr) = connection;
                streams::stream_http1(rt::TokioIo::new(client), client_addr.clone(), t.clone())
                    .await?;
                ewe_logs::info!(
                    "Finished serving::http1 client: {} from {} to {}",
                    client_addr.clone(),
                    t.source,
                    t.destination,
                );
                Ok(())
            }
            _ => Err(Box::new(ProxyError::TunnelNotSupported(self)).into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxyRemote(ProxyType);

// -- Constructors

impl ProxyRemote {
    #[must_use]
    pub fn new(proxy_type: ProxyType) -> Self {
        Self(proxy_type)
    }

    #[must_use]
    pub fn shared(proxy_type: ProxyType) -> sync::Arc<Self> {
        sync::Arc::new(Self::new(proxy_type))
    }
}

// -- Operator trait implementation

impl Operator for sync::Arc<ProxyRemote> {
    fn run(&self, signal: broadcast::Receiver<()>) -> JoinHandle<()> {
        let handler = self.clone();
        tokio::spawn(async move { handler.stream(signal).await })
    }
}

// -- Implementation details

impl ProxyRemote {
    pub async fn stream(&self, mut sig: broadcast::Receiver<()>) -> Result<()> {
        ewe_logs::info!("Streaming for proxy: {}", self.0,);

        let (kill_sender, kill_receiver) = oneshot::channel::<()>();

        let kill_thread = tokio::task::spawn_blocking(move || {
            _ = sig.blocking_recv().expect("should receive kill signal");
            kill_sender.send(()).expect("should send kill signal");
        });

        tokio::select! {

            res = async {

                match &self.0 {
                    ProxyType::Http1(t) => {
                        ewe_logs::info!("Creating TCPListener for {} (addr_str: {}, protocol: Http1) to {}", t.source, t.source.to_string(), t.destination);
                        let source_listener = net::TcpListener::bind(t.source.to_string()).await?;

                        loop {
                            let proxy_elem = self.0.clone();
                            match source_listener.accept().await {
                                Ok(connection) => {
                                    tokio::spawn(async move {
                                        if let Err(err) = proxy_elem.clone().stream_http1(connection).await {
                                            ewe_logs::error!(
                                                "Failed to serve http1 request: {}  - {:?}",
                                                proxy_elem.clone(),
                                                err,
                                            );
                                        }
                                    });
                                    continue;
                                },
                                Err(err) => {
                                    ewe_logs::error!(
                                        "Failed to get new client connection {:?}",
                                        err,
                                    );
                                    break;
                                }
                            };

                        }
                        Ok(())
                    },
                    ProxyType::Tunnel(t) => {
                        ewe_logs::info!("Creating TCPListener for {} (addr_str: {}, protocol: tunnel) to {}", t.source, t.source.to_string(), t.destination);
                        let source_listener = net::TcpListener::bind(t.source.to_string()).await?;

                        loop {
                            let proxy_elem = self.0.clone();
                            match source_listener.accept().await {
                                Ok(connection) => {
                                    tokio::spawn(async move {
                                        if let Err(err) = proxy_elem.clone().tunnel_connection(connection).await {
                                            ewe_logs::error!(
                                                "Failed to serve tcp tunnel request: {}  - {:?}",
                                                proxy_elem.clone(),
                                                err,
                                            );
                                        }
                                    });
                                    continue;
                                },
                                Err(err) => {
                                    ewe_logs::error!(
                                        "Failed to get new client connection {:?}",
                                        err,
                                    );
                                    break;
                                }
                            };

                        }
                        Ok(())
                    },
                    _ => Err(Box::new(ProxyError::TunnelNotSupported(self.0.clone())).into())
                }

            } => {
                res
            }

            _ = kill_receiver => {
                kill_thread.await.expect("should have died correctly");
                Ok(())
            }
        }
    }
}

pub struct StreamTCPApp {
    wait_for_binary_secs: time::Duration,
    proxy_type: ProxyType,
}

// -- Constructor

impl StreamTCPApp {
    pub fn shared(wait_for_binary_secs: time::Duration, proxy_type: ProxyType) -> sync::Arc<Self> {
        sync::Arc::new(Self {
            wait_for_binary_secs,
            proxy_type,
        })
    }
}

// -- Binary starter
impl StreamTCPApp {
    fn run_proxy(&self, sig: broadcast::Receiver<()>) -> JoinHandle<()> {
        let proxy_server = ProxyRemote::shared(self.proxy_type.clone());
        proxy_server.run(sig)
    }
}

// -- Operator implementation

impl Operator for sync::Arc<StreamTCPApp> {
    fn run(&self, signal: broadcast::Receiver<()>) -> JoinHandle<()> {
        let wait_for = self.wait_for_binary_secs.clone();

        let pt = self.proxy_type.clone();
        let handler = self.clone();

        tokio::spawn(async move {
            tokio::time::sleep(wait_for).await;
            let proxy_handler = handler.run_proxy(signal);

            ewe_logs::info!("Booting up proxy server proxy_type={:?}", pt);

            match proxy_handler.await? {
                Ok(_) => Ok(()),
                Err(err) => {
                    ewe_logs::error!("Failed to properly end tcp proxy: {:?}", err);
                    Err(Box::new(ProxyError::FailedProxyConnection).into())
                }
            }
        })
    }
}
