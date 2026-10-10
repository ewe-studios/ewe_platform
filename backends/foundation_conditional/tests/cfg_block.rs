//! `cfg_block!` gates on the invoking crate's features. This test uses this
//! crate's own `debug_block` feature, so it holds with and without
//! `--features debug_block`.

use foundation_conditional::cfg_block;

#[test]
fn cfg_block_follows_the_named_feature() {
    let compiled_in = std::cell::Cell::new(false);
    cfg_block!("debug_block" => {
        compiled_in.set(true);
    });
    assert_eq!(compiled_in.get(), cfg!(feature = "debug_block"));
}
