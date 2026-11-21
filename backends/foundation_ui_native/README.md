# Native UI core

The native user interface (UI) core runs compiled Rust actions and sends remote actions through an application transport.
Unified Foreign Function Interface (`UniFFI`) generates Swift and Kotlin bindings from the same compiled library.
One call carries one complete frame. A host can keep a socket open and receive later frames separately.

## Start the core

Use the remote factory in an app that gets its actions from a server. Declare the server action names at startup.
The transport supplies network access, credentials, a request timeout and any explicit retry policy.
A request or reply is an unprefixed EWF1 buffer. EWF1 is the wire identifier.
Remove the four-byte transport length when reading a length-prefixed HTTP response.

```swift
import EweCore
import Foundation

func save(transport: HostTransport, signedRow: Data) async throws -> Data? {
    let core = try NativeCore.remote(platform: .ios, names: ["save"], transport: transport)
    let hello = core.hello()
    if let caps = try await transport.send(request: hello) {
        try core.applyCapsFrame(frame: caps)
    }
    let input = EventInput(nodeId: 42, eventName: "tap", value: "", checked: false,
                          parentNode: 40, state: signedRow)
    return try await core.trigger(reference: "server:save", event: input)
}
```

A Kotlin app uses the same remote factory. The generated `trigger` method suspends while the action runs.
Both hosts use the state bytes on the event's own node. Unmarked nodes send an empty byte array.
A successful socket handoff returns no immediate frame. The host routes later messages through its renderer.
The core validates incoming message frames and rejects web operations before returning an immediate reply.

## Run local Rust actions

An application Rust crate registers its actions and exposes its core factory to Swift or Kotlin.
Install the signer and the render target in the host context before constructing the core.
Use the same signer and binding policy to render the page and handle its actions.

```rust
use std::sync::Arc;
use foundation_context::Ctx;
use foundation_ui_native::{command, CoreConfig, NativeCore, NativePlatform};
use foundation_ui_runtime::{action, action::{ActionRegistry, ActionResult, Context, Interaction}};
use foundation_ui_runtime::state::{install_state_signer, HmacStateSigner, StateConfig};
use foundation_ui_wire::writer::WriterConfig;

#[action(host)]
async fn feedback(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    ctx.receiver().render_op(command::haptic());
    Ok(())
}

fn create(ctx: Ctx, key: [u8; 32]) -> Result<Arc<NativeCore>, Box<dyn std::error::Error>> {
    install_state_signer(&ctx, Arc::new(HmacStateSigner::new(key, StateConfig::default())?));
    let mut registry = ActionRegistry::new();
    registry.register(feedback)?;
    Ok(NativeCore::new(CoreConfig {
        ctx, registry: Arc::new(registry), platform: NativePlatform::Ios,
        writer: WriterConfig::default(),
    }, None))
}
```

`command::open_url` and `command::share` carry one named text argument.
The host decides which operating system command to run.
Install a `FrameListener` before local Rust logic calls `push_ops`.
Each successful push gets one sequence number. Failed writes and absent listeners consume no number.
The optional `app` feature supplies `CoreSink` for local signals.

## Generate and check bindings

Run these commands from the workspace root.

```sh
scripts/ui_native/generate-bindings.sh --out target/native-ui/bindings
scripts/ui_native/generate-bindings.sh --check --out target/native-ui/bindings
cargo test --profile uat -p foundation_ui_native --features app
cargo check --profile uat -p foundation_ui_native --lib --target aarch64-apple-ios
cargo check --profile uat -p foundation_ui_native --lib --target aarch64-linux-android
```

The generator writes Swift to `swift/` and Kotlin to `kotlin/` under the output directory.
A check without an output directory compares two fresh generations.
The sibling Swift and Kotlin packages consume these bindings and the native library built from the same revision.
The runtime types use the Swift module `EweCore` and the Kotlin package `io.github.ewe_studios.ewe.core`.

## Fixtures and wire messages

The remote action test uses `frame_ops_counter.bin` as the transport reply and compares all returned bytes.
The tests build additional messages with the generated Rust tables. They include mobile commands, behaviours, nested web operations and invalid replies.
See the [native acceptance tests](tests/core_tests.rs) and the [wire message and fixture guide](../foundation_ui_wire/README.md).
