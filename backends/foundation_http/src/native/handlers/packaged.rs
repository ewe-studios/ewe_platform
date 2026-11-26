//! Resolve declared resource names when a request arrives.
use super::static_file::{StaticFileConfig, StaticFileHandler};
use crate::{
    native::{
        serve::Serve,
        writer::{
            render_head, send_not_found, send_server_error, write_response, ResponseWriter,
            WriterConfig,
        },
    },
    shared::{context::Ctx, router::Route, serve::ConnectionResult},
};
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::{
    netcap::RawStream,
    shared::http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod},
};
use std::{
    io,
    path::{Path, PathBuf},
};

/// The bytes or live file selected by an application resource provider.
pub enum PackagedFile {
    Live(PathBuf),
    Embedded(&'static [u8]),
}

/// A build-generated mapping from logical resource names to file contents.
pub trait PackagedResourceSet: Send + Sync + 'static {
    /// Look up one relative name in the package.
    ///
    /// # Errors
    /// Returns an error if the host cannot locate its resource directory.
    fn lookup(&self, name: &str) -> io::Result<Option<PackagedFile>>;
}

/// A single application route that serves the declared resource set.
pub struct ResourceFileHandler<R> {
    pub(super) resources: R,
    pub(super) directory: Option<PathBuf>,
}

impl<R> ResourceFileHandler<R> {
    /// Supply the live resource directory when embedding a server in another executable.
    #[must_use]
    pub fn with_public_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.directory = Some(directory.into());
        self
    }
}

impl<R> Route for ResourceFileHandler<R> {
    const METHOD: SimpleMethod = SimpleMethod::GET;
    const PATH: &'static str = "/*";
}

impl<R: PackagedResourceSet> Serve for ResourceFileHandler<R> {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let url = req.request_url.url.split('?').next().unwrap_or_default();
        let name = match url.strip_prefix('/') {
            Some("") => "index.html",
            Some(path) => path,
            None => return send_not_found(conn, &ctx),
        };
        match self.resources.lookup(name) {
            Ok(Some(PackagedFile::Live(path))) => StaticFileHandler::file(
                self.directory.as_ref().map_or(path, |dir| dir.join(name)),
                StaticFileConfig::default(),
            )
            .serve(ctx, req, conn),
            Ok(Some(PackagedFile::Embedded(body))) => {
                let mime = foundation_netio::shared::mime::from_path(Path::new(name))
                    .unwrap_or(foundation_netio::shared::mime::OCTET_STREAM);
                let headers = [(SimpleHeader::CONTENT_TYPE, mime.to_owned())];
                match render_head(200, &headers, body.len()) {
                    Ok(head) => ConnectionResult::Async(Box::pin(write_response(
                        ResponseWriter::new(conn, WriterConfig::default(), ctx),
                        head,
                        body,
                    ))),
                    Err(error) => {
                        tracing::error!(%error,"Cannot render packaged resource headers");
                        send_server_error(conn, None, &ctx)
                    }
                }
            }
            Ok(None) => send_not_found(conn, &ctx),
            Err(error) => {
                tracing::error!(%error,"Cannot locate packaged file");
                send_server_error(conn, None, &ctx)
            }
        }
    }
}
