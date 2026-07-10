//! russh (pure Rust, async/tokio-native) backend.
//! Enable with `features = ["russh-backend"]`.

use std::path::Path;
use std::time::Instant;

use async_trait::async_trait;
use russh::*;
use russh::keys::*;
use russh_sftp::client::SftpSession;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::backends::Backend;
use crate::command::{Command, CommandResult};
use crate::host::Host;

#[derive(Default)]
pub struct RusshBackend;

impl RusshBackend {

    pub async fn execute_async(&self, host: &Host, cmd: &Command) -> Result<CommandResult, String> {
        let shell_cmd = cmd.to_shell_command();
        let key_path = host.key_paths.first()
            .ok_or("russh requires an explicit key path".to_string())?;
        let auth_key = load_secret_key(key_path, None)
            .map_err(|e| format!("load key: {e}"))?;

        struct Handler;
        #[async_trait]
        impl client::Handler for Handler {
            type Error = russh::Error;
            async fn check_server_key(
                &mut self,
                _key: &key::PublicKey,
            ) -> Result<bool, Self::Error> { Ok(true) }
        }

        let config = std::sync::Arc::new(client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(30)),
            ..<_>::default()
        });

        let mut session = client::connect(config, (host.hostname.as_str(), host.port), Handler)
            .await.map_err(|e| format!("connect: {e}"))?;

        let auth_ok = session.authenticate_publickey(host.user.clone(), std::sync::Arc::new(auth_key))
            .await.map_err(|e| format!("auth: {e}"))?;
        if !auth_ok { return Err("russh authentication rejected".to_string()); }

        let mut channel = session.channel_open_session()
            .await.map_err(|e| format!("channel: {e}"))?;
        let cmd_bytes = shell_cmd.as_bytes().to_vec();
        channel.exec(true, cmd_bytes)
            .await.map_err(|e| format!("exec: {e}"))?;

        let start = Instant::now();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code: i32 = -1;

        loop {
            let Some(msg) = channel.wait().await else { break };
            match msg {
                ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
                ChannelMsg::ExtendedData { ref data, .. } => stderr.extend_from_slice(data),
                ChannelMsg::ExitStatus { exit_status } => exit_code = exit_status as i32,
                _ => {}
            }
        }

        session.disconnect(Disconnect::ByApplication, "", "English").await.ok();
        Ok(CommandResult {
            exit_code,
            stdout: String::from_utf8_lossy(&stdout).to_string(),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
            runtime: start.elapsed(),
            host: format!("{}@{}", host.user, host.hostname),
        })
    }

    pub async fn upload_async(&self, host: &Host, local: &Path, remote: &Path) -> Result<(), String> {
        let data = std::fs::read(local).map_err(|e| format!("read local: {e}"))?;
        let key_path = host.key_paths.first().ok_or("russh requires key path")?;
        let auth_key = load_secret_key(key_path, None).map_err(|e| format!("load key: {e}"))?;

        struct Handler;
        #[async_trait]
        impl client::Handler for Handler {
            type Error = russh::Error;
            async fn check_server_key(&mut self, _: &key::PublicKey) -> Result<bool, Self::Error> { Ok(true) }
        }

        let config = std::sync::Arc::new(client::Config::default());
        let mut session = client::connect(config, (host.hostname.as_str(), host.port), Handler)
            .await.map_err(|e| format!("connect: {e}"))?;
        session.authenticate_publickey(host.user.clone(), std::sync::Arc::new(auth_key))
            .await.map_err(|e| format!("auth: {e}"))?;

        let channel = session.channel_open_session().await.map_err(|e| format!("channel: {e}"))?;
        channel.request_subsystem(true, "sftp").await.map_err(|e| format!("sftp: {e}"))?;
        let sftp = SftpSession::new(channel.into_stream()).await
            .map_err(|e| format!("sftp session: {e}"))?;

        let remote_path = std::path::PathBuf::from(remote);
        let filename = remote_path.file_name()
            .ok_or("invalid remote path")?
            .to_str()
            .ok_or("non-UTF8 path")?;

        let mut file = sftp.create(filename).await.map_err(|e| format!("sftp create: {e}"))?;
        file.write_all(&data).await.map_err(|e| format!("sftp write: {e}"))?;
        file.shutdown().await.map_err(|e| format!("sftp shutdown: {e}"))?;

        session.disconnect(Disconnect::ByApplication, "", "").await.ok();
        Ok(())
    }

    pub async fn download_async(&self, host: &Host, remote: &Path, local: &Path) -> Result<(), String> {
        let key_path = host.key_paths.first().ok_or("russh requires key path")?;
        let auth_key = load_secret_key(key_path, None).map_err(|e| format!("load key: {e}"))?;

        struct Handler;
        #[async_trait]
        impl client::Handler for Handler {
            type Error = russh::Error;
            async fn check_server_key(&mut self, _: &key::PublicKey) -> Result<bool, Self::Error> { Ok(true) }
        }

        let config = std::sync::Arc::new(client::Config::default());
        let mut session = client::connect(config, (host.hostname.as_str(), host.port), Handler)
            .await.map_err(|e| format!("connect: {e}"))?;
        session.authenticate_publickey(host.user.clone(), std::sync::Arc::new(auth_key))
            .await.map_err(|e| format!("auth: {e}"))?;

        let channel = session.channel_open_session().await.map_err(|e| format!("channel: {e}"))?;
        channel.request_subsystem(true, "sftp").await.map_err(|e| format!("sftp: {e}"))?;
        let sftp = SftpSession::new(channel.into_stream()).await
            .map_err(|e| format!("sftp session: {e}"))?;

        let filename = remote.file_name().ok_or("invalid remote path")?
            .to_str().ok_or("non-UTF8 path")?;
        let mut file = sftp.open(filename).await.map_err(|e| format!("sftp open: {e}"))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await.map_err(|e| format!("sftp read: {e}"))?;
        std::fs::write(local, data).map_err(|e| format!("write local: {e}"))?;

        session.disconnect(Disconnect::ByApplication, "", "").await.ok();
        Ok(())
    }
}

impl Backend for RusshBackend {
    fn execute(&self, host: &Host, cmd: &Command) -> Result<CommandResult, String> {
        futures_lite::future::block_on(self.execute_async(host, cmd))
    }
    fn upload(&self, host: &Host, local: &Path, remote: &Path) -> Result<(), String> {
        futures_lite::future::block_on(self.upload_async(host, local, remote))
    }
    fn download(&self, host: &Host, remote: &Path, local: &Path) -> Result<(), String> {
        futures_lite::future::block_on(self.download_async(host, remote, local))
    }
}
