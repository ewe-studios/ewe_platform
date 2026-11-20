# User interface state runtime

This crate seals and opens the state of user interface (UI) components.

- The hash-based message authentication code (HMAC) covers the state fields with Secure Hash Algorithm 256 (SHA-256).
- The codec reads the clock and random number generator (RNG) from the host context.
- For private state, the codec encrypts the body with ChaCha20-Poly1305 before it calculates the tag.
- The host supplies the keys. A previous key can open its states until they expire.
- A trusted device can use the plain codec. The plain codec uses no tag or encryption.

This example receives a context and a key from the host. It returns the verified state body.

```rust
use foundation_context::Ctx;
use foundation_ui_runtime::state::{
    Expect, HmacStateCodec, Session, StateCodec, StateConfig, StateError, StateKey,
    StateKeyRing, UnsealedState,
};

fn restore_counter(ctx: &Ctx, key: StateKey, session_id: &str) -> Result<Vec<u8>, StateError> {
    let codec = HmacStateCodec::new(StateKeyRing::new(key), StateConfig::default());
    let session = Session::new(session_id)?;
    let state = UnsealedState {
        format_version: 1,
        component_type: 7,
        root_node: 40,
        body: vec![1, 0, 0, 0],
        private: true,
    };
    let sealed = codec.seal(ctx, &session, &state)?;
    let expect = Expect { component_type: 7, format_version: 1 };
    Ok(codec.open(ctx, &session, &sealed, expect)?.body)
}
```

The dispatcher checks the failure limiter before it opens state. It records a failure after an unsuccessful open.
The host bounds the number of caller keys when it uses the in-memory limiter.
A host with many callers supplies its own limiter backed by a store.

Run the codec tests and check the wire fixture.

```sh
cargo test -p foundation_ui_runtime --profile uat
cargo test -p foundation_ui_wire --profile uat fixtures_match_built_values
```
