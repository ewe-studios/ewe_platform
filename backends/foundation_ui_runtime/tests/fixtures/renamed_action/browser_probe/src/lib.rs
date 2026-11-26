pub const MESSAGE: &[u8] = b"EWE_ACTION_ARTIFACT_BROWSER";

/// Browser futures can retain a single-threaded value across suspension.
pub async fn next_count(current: u32) -> u32 {
    #[cfg(target_family = "wasm")]
    let current = std::rc::Rc::new(current);
    std::future::ready(()).await;
    current.saturating_add(1)
}
