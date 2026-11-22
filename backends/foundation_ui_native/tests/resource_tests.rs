use foundation_context::Ctx;
use foundation_resources::{ReadConfig, ResourceError, Resources};
use foundation_ui_native::{
    resources::{HostResourceFile, HostResourceSource, MobileReader},
    CoreError,
};
use futures_util::StreamExt;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Source(Arc<AtomicUsize>);
impl HostResourceSource for Source {
    fn open_resource_file(&self, name: String) -> Result<Option<Arc<dyn HostResourceFile>>, CoreError> {
        Ok(match name.as_str() {
            "icon" | "broken" => Some(Arc::new(File {
                reads: AtomicUsize::new(0),
                closed: self.0.clone(),
                broken: name == "broken",
            })),
            _ => None,
        })
    }
}

struct File {
    reads: AtomicUsize,
    closed: Arc<AtomicUsize>,
    broken: bool,
}
impl HostResourceFile for File {
    fn read(&self, capacity: u32) -> Result<Vec<u8>, CoreError> {
        assert_eq!(capacity, 2);
        match self.reads.fetch_add(1, Ordering::SeqCst) {
            0 => Ok(vec![1, 2]),
            _ if self.broken => Err(CoreError::Transport {
                detail: "read failed".into(),
            }),
            _ => Ok(Vec::new()),
        }
    }
    fn close_file(&self) -> Result<(), CoreError> {
        self.closed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn mobile_reads_close_after_end_error_and_early_drop() {
    let ctx = Ctx::host();
    let closed = Arc::new(AtomicUsize::new(0));
    let config = ReadConfig::default().with_chunk_bytes(2.try_into().unwrap());
    Resources::install(
        &ctx,
        Arc::new(MobileReader::new(Arc::new(Source(closed.clone())), config)),
    );
    futures_lite::future::block_on(async {
        let resources = Resources::from_context(&ctx);
        assert!(matches!(
            resources.open("absent").await,
            Err(ResourceError::Missing { .. })
        ));
        for name in ["icon", "broken", "icon"] {
            let mut stream = resources.open(name).await.unwrap();
            assert_eq!(stream.next().await.unwrap().unwrap(), [1, 2]);
            match closed.load(Ordering::SeqCst) {
                0 => assert!(stream.next().await.is_none()),
                1 => assert!(matches!(
                    stream.next().await,
                    Some(Err(ResourceError::Read { .. }))
                )),
                _ => drop(stream),
            }
        }
    });
    assert_eq!(closed.load(Ordering::SeqCst), 3);
}
