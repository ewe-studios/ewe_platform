//! Persist the mobile outbox and send its local preview before the network request.
use crate::outbox;
use foundation_context::Ctx;
use foundation_ui_native::{CoreError, HostTransport, NativeCore};
use foundation_ui_runtime::action::{prepare_messages, Context};
use foundation_ui_wire::{read_frame, MountSurface, WireLimits};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, Weak};

pub(super) struct FileStorage(pub(super) PathBuf);

#[async_trait::async_trait]
impl crate::PendingStorage for FileStorage {
    async fn load(&self) -> Result<Vec<u8>, String> {
        let path = self.0.clone();
        blocking::unblock(move || match std::fs::read(path) {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(error) => Err(error.to_string()),
        })
        .await
    }

    async fn save(&self, contents: Vec<u8>) -> Result<(), String> {
        let path = self.0.clone();
        blocking::unblock(move || -> std::io::Result<()> {
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::other("Missing outbox directory"))?;
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            file.write_all(&contents)?;
            file.as_file().sync_all()?;
            file.persist(&path).map_err(|e| e.error)?;
            std::fs::File::open(parent)?.sync_all()
        })
        .await
        .map_err(|e| e.to_string())
    }
}

pub(super) struct Sender {
    pub(super) ctx: Ctx,
    pub(super) transport: Arc<dyn HostTransport>,
    pub(super) core: OnceLock<Weak<NativeCore>>,
}

fn failed(error: impl std::fmt::Display) -> CoreError {
    CoreError::Transport {
        detail: error.to_string(),
    }
}

impl Sender {
    fn publish(&self, context: Context) -> Result<(), CoreError> {
        let messages =
            prepare_messages(&self.ctx, context.into_receiver().drain()).map_err(failed)?;
        if messages.is_empty() {
            return Ok(());
        }
        let operations = messages
            .into_iter()
            .map(|message| {
                message
                    .operation
                    .map(|op| *op)
                    .ok_or_else(|| failed("A pending preview requires a view operation"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.core
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| failed("The Todo screen closed"))?
            .push_ops(operations)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl HostTransport for Sender {
    async fn send(&self, original: Vec<u8>) -> Result<Option<Vec<u8>>, CoreError> {
        let context = Context::for_render_on(self.ctx.clone(), MountSurface::Native);
        let plan = outbox::prepare(&context, original).await.map_err(failed)?;
        self.publish(context)?;
        let Some(request) = plan.request else {
            return Ok(None);
        };
        let result = self.transport.send(request).await;
        let failure = match &result {
            Err(_) => Some(None),
            Ok(Some(bytes)) => match read_frame(bytes, &WireLimits::default()) {
                Err(_) => Some(None),
                Ok(frame) => frame
                    .kind_as_frame_message()
                    .and_then(|body| body.messages())
                    .and_then(|messages| {
                        messages.iter().find_map(|message| {
                            let error = message.control()?.error()?;
                            Some(
                                (error.code() == foundation_ui_wire::ErrorCode::InvalidInput).then(
                                    || error.message().unwrap_or("Check the title").to_owned(),
                                ),
                            )
                        })
                    }),
            },
            Ok(None) => None,
        };
        if let (Some(key), Some(validation)) = (plan.key, failure) {
            let context = Context::for_render_on(self.ctx.clone(), MountSurface::Native);
            outbox::failed(&context, &key, validation)
                .await
                .map_err(failed)?;
            self.publish(context)?;
        }
        result
    }
}
