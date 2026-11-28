//! A host stores the store in its context. The two must not keep each other alive.

use std::sync::{Arc, Weak};

use foundation_context::Ctx;
use foundation_core::durable::{DurableStream, DurableStreamVfs, MemoryVfs};

#[test]
fn given_a_store_in_its_context_when_the_context_drops_then_the_vfs_drops() {
    let ctx = Ctx::host();
    let vfs = Arc::new(MemoryVfs::new());
    let weak: Weak<MemoryVfs> = Arc::downgrade(&vfs);

    let stream_vfs: Arc<dyn DurableStreamVfs> = vfs.clone();
    let socket = DurableStream::new(stream_vfs, &ctx);
    let _ = ctx.get_or_insert(socket);

    // The context is the only remaining owner of the VFS. Drop the local handle now.
    drop(vfs);
    assert!(
        weak.upgrade().is_some(),
        "the context must keep the store, and so the VFS, alive"
    );

    drop(ctx);

    // A cycle leaves the context inner block alive with the store and the VFS inside it.
    assert!(
        weak.upgrade().is_none(),
        "the VFS outlived its context: DurableStream keeps the Ctx that keeps it, so the Arc cycle leaks"
    );
}
