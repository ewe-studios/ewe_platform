#![cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
use foundation_context::Ctx;
use foundation_resources::{
    DirectoryReader, OpenFuture, ReadConfig, Resource, ResourceError, ResourceName, ResourceReader,
    Resources,
};
use futures_util::StreamExt;
use std::{fs, num::NonZeroUsize, sync::Arc};

#[test]
fn host_context_streams_the_named_file_in_bounded_chunks() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("art.txt"), b"abcde").unwrap();
    let ctx = Ctx::host();
    futures_lite::future::block_on(async {
        assert!(matches!(
            Resources::from_context(&ctx).open("art.txt").await,
            Err(ResourceError::MissingHost)
        ));
        Resources::install(
            &ctx,
            Arc::new(DirectoryReader::new(
                root.path().into(),
                ReadConfig::default().with_chunk_bytes(NonZeroUsize::new(2).unwrap()),
            )),
        );
        let resources = Resources::from_context(&ctx);
        let mut file = resources.open("art.txt").await.unwrap();
        assert_eq!(file.next().await.unwrap().unwrap(), b"ab");
        assert_eq!(file.next().await.unwrap().unwrap(), b"cd");
        assert_eq!(file.next().await.unwrap().unwrap(), b"e");
        assert!(file.next().await.is_none());
        assert!(matches!(
            resources.open("missing").await,
            Err(ResourceError::Missing { .. })
        ));
        assert!(matches!(
            resources.open("../outside").await,
            Err(ResourceError::InvalidName(_))
        ));
    });
}

struct Interrupted;
impl ResourceReader for Interrupted {
    fn open(&self, name: ResourceName) -> OpenFuture<'_> {
        Box::pin(async move {
            Ok(Box::pin(futures_util::stream::iter([
                Ok(vec![1_u8]),
                Err(ResourceError::Read {
                    name: name.as_str().into(),
                    reason: "interrupted".into(),
                }),
            ])) as Resource)
        })
    }
}

#[test]
fn a_read_failure_remains_visible_after_open() {
    let ctx = Ctx::host();
    Resources::install(&ctx, Arc::new(Interrupted));
    futures_lite::future::block_on(async {
        let mut stream = Resources::from_context(&ctx).open("image").await.unwrap();
        assert_eq!(stream.next().await.unwrap().unwrap(), [1]);
        assert!(matches!(
            stream.next().await,
            Some(Err(ResourceError::Read { .. }))
        ));
    });
}
