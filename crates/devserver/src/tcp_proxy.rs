use core::fmt;
use crossbeam::channel;
use std::net::SocketAddr;
use std::{sync, time};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

use tokio::{net, sync::broadcast};

use crate::types::{JoinHandle, Result};
use crate::Operator;

const DEFAULT_BUF_SIZE: usize = 1024;

// -- Errors

#[derive(Debug, derive_more::From)]
pub enum ProxyError {
    FailedProxyConnection,
    ConnectionDrop,
    StreamingFailed,
}

impl std::error::Error for ProxyError {}

impl core::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProxyRemoteConfig {
    pub addr: String,
    pub port: usize,
}

// -- Constructors

impl ProxyRemoteConfig {
    #[must_use]
    pub fn new(addr: String, port: usize) -> Self {
        Self { addr, port }
    }
}

// -- Debug Display

impl fmt::Display for ProxyRemoteConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProxyRemote {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,
}

async fn stream_writes<R, W>(
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
                ewe_logs::error!("Failed to write data to destination: {:?}", e)
            }
        }
    }

    Ok(copied)
}

async fn stream_client(
    mut source: net::TcpStream,
    source_addr: SocketAddr,
    destination_config: ProxyRemoteConfig,
) -> Result<()> {
    ewe_logs::info!(
        "Starting streaming between client addr {} and destination {} ",
        source_addr,
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
            ewe_logs::info!("Copied total bytes: {} from source to destination", copied);
        }
        Err(err) => {
            ewe_logs::error!("Failed in data transmission to destination: {:?}", err);
        }
    };

    match destination_to_source {
        Ok(copied) => {
            ewe_logs::info!("Copied total bytes: {} from destination to source", copied);
        }
        Err(err) => {
            ewe_logs::error!("Failed in data transmission to source: {:?}", err);
        }
    };

    Ok(())
}

// -- Constructors

impl ProxyRemote {
    #[must_use]
    pub fn new(source: ProxyRemoteConfig, destination: ProxyRemoteConfig) -> Self {
        Self {
            source,
            destination,
        }
    }

    #[must_use]
    pub fn shared(source: ProxyRemoteConfig, destination: ProxyRemoteConfig) -> sync::Arc<Self> {
        sync::Arc::new(Self::new(source, destination))
    }
}

// -- Operator trait implementation

impl Operator for sync::Arc<ProxyRemote> {
    fn run(&self, signal: channel::Receiver<()>) -> JoinHandle<()> {
        let handler = self.clone();
        tokio::spawn(async move { handler.stream(signal).await })
    }
}

// -- Implementation details

impl ProxyRemote {
    pub async fn stream(&self, sig: channel::Receiver<()>) -> Result<()> {
        ewe_logs::info!(
            "Streaming data from {} to {}",
            self.source,
            self.destination
        );

        let (kill_sender, kill_receiver) = oneshot::channel::<()>();

        let kill_thread = tokio::task::spawn_blocking(move || {
            _ = sig.recv().expect("should receive kill signal");
            kill_sender.send(()).expect("should send kill signal");
        });

        tokio::select! {

            res = async {
                let source_addr_str = self.source.to_string();
                let source_listener = net::TcpListener::bind(source_addr_str).await?;
                ewe_logs::info!("Creating TCPListener for {}", source_listener.local_addr().expect("listener should have local address"));

                loop {
                    let (client, client_addr) = source_listener.accept().await?;

                    let remote_config = self.destination.clone();
                    tokio::spawn(async move {
                        match stream_client(client, client_addr.clone(), remote_config.clone()).await {
                            Ok(_) => ewe_logs::info!("Finished serving client: {}", client_addr.clone()),
                            Err(err) => ewe_logs::error!(
                                "Failed to serve client: {} - {:?}",
                                client_addr.clone(),
                                err,
                            ),
                        }
                    });
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
    source_config: ProxyRemoteConfig,
    destination_config: ProxyRemoteConfig,
}

// -- Constructor

impl StreamTCPApp {
    pub fn shared(
        wait_for_binary_secs: time::Duration,
        source: ProxyRemoteConfig,
        destination: ProxyRemoteConfig,
    ) -> sync::Arc<Self> {
        sync::Arc::new(Self {
            wait_for_binary_secs,
            source_config: source,
            destination_config: destination,
        })
    }
}

// -- Binary starter
impl StreamTCPApp {
    fn run_proxy(&self, sig: channel::Receiver<()>) -> JoinHandle<()> {
        let proxy_server =
            ProxyRemote::shared(self.source_config.clone(), self.destination_config.clone());
        proxy_server.run(sig)
    }
}

// -- Operator implementation

impl Operator for sync::Arc<StreamTCPApp> {
    fn run(&self, signal: channel::Receiver<()>) -> JoinHandle<()> {
        let wait_for = self.wait_for_binary_secs.clone();
        let source = self.source_config.clone();
        let destination = self.destination_config.clone();

        let handler = self.clone();

        tokio::spawn(async move {
            tokio::time::sleep(wait_for).await;
            let proxy_handler = handler.run_proxy(signal);

            ewe_logs::info!(
                "Booting up proxy server for source={:?} through destination={:?}",
                source,
                destination
            );

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
