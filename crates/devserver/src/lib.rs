// Implements the core functionality to manage and serve a local
// ewe platform web application for local development.

use core::fmt;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::{
    net::{self},
    sync::broadcast,
};

const DEFAULT_BUF_SIZE: usize = 1024;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

type Result<T> = std::result::Result<T, BoxedError>;

#[derive(Debug, Default, Clone)]
pub struct ProxyRemoteConfig {
    pub addr: String,
    pub port: usize,
}

impl ProxyRemoteConfig {
    #[must_use]
    pub fn new(addr: String, port: usize) -> Self {
        Self { addr, port }
    }
}

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

impl ProxyRemote {
    #[must_use]
    pub fn new(source: ProxyRemoteConfig, destination: ProxyRemoteConfig) -> Self {
        Self {
            source,
            destination,
        }
    }

    pub async fn stream(&self) -> Result<()> {
        let source_addr_str = self.source.to_string();
        let source_listener = net::TcpListener::bind(source_addr_str).await?;

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
    }
}
