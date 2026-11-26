# Local graphics

A graphics processing unit (GPU) runs computation and draws images. The host owns this optional service.
A shader is a program that runs on the GPU. A surface presents its output to one view.
The service keeps one device and one command queue on their owning thread or browser event loop.
Call `shutdown().await` from the host before it exits. This waits for accepted jobs and releases graphics resources.

## Run a shader from Rust

Install the service when the host creates its context. Local actions read the same service from that context.
The first job that needs a device opens it. Concurrent jobs share that device.

```rust
use foundation_context::Ctx;
use foundation_ui_gpu::{GpuConfig, GpuError, GpuService};

async fn compute(ctx: &Ctx, shader: String) -> Result<Vec<u32>, GpuError> {
    let graphics = GpuService::install(ctx, GpuConfig::default())?;
    graphics.run(move |owner| Box::pin(async move {
        owner.compute_words(&shader, &[0, 2, 7], 3).await
    })).await
}
```

The shader has a writable storage buffer at group zero and binding zero. Its entry point is `main`.
The example shader doubles each word and adds one. It returns 1, 5, and 15. [Shader](tests/shaders/double.wgsl)

A job can also use `owner.device().await?` to get the wgpu adapter, device, and queue.
Use the adapter to inspect supported features and limits. Set the required features and limits in `GpuConfig` before installation.
Keep graphics resources on the owner. Return owned data to the caller.

## Draw to a view

Declare a target with `html! { <DrawingSurface id="preview" width="200px" height="100px"/> }`.
The renderer retains the platform target. Its name lets local Rust find the surface after the renderer attaches it.

```rust
use foundation_ui_gpu::{wgpu::Color, GpuError, GpuService};

async fn clear_preview(graphics: &GpuService) -> Result<(), GpuError> {
    graphics.run(|owner| Box::pin(async {
        let surface = owner.named_surface("preview").ok_or(GpuError::StaleSurface)?;
        owner.clear(surface, Color::BLUE).await
    })).await
}
```

A custom renderer implements `RenderHost`. It supplies a retained target and its physical size.
The host calls `attach`, `resize`, and `detach`. Zero width or height suspends presentation.
`present` gives a local job the current texture. The service presents it after the job submits its commands.
A removed target rejects late work. Other targets and compute jobs stay active.
Remote frames can create a drawing control. Only the local application can submit GPU jobs.

## Handle errors

An unsupported device, lost device, suspended target, and stale handle each have a distinct error.
Initialization failures stay cached. Call `reinitialize` to release old resources and try again.
Reattach each active target after reinitialization. Dropping a queued job before it starts cancels its work.
Commands that already reached the device finish normally.

## Set up the browser

Build the WebAssembly (Wasm) module and its JavaScript bindings from the repository root.

```sh
cargo build --profile uat --target wasm32-unknown-unknown -p foundation_ui_gpu
mise exec -- wasm-bindgen --target web target/wasm32-unknown-unknown/uat/foundation_ui_gpu.wasm --out-dir target/ui-gpu-web
```

Use a secure browser context with WebGPU support. Pass the same graphics object to the frame renderer.

```js
import init, {BrowserGraphics} from "./foundation_ui_gpu.js";
await init();
const graphics = new BrowserGraphics();
installFrameRuntime(wasmUi, {graphics});
```

A browser application built in Rust calls `BrowserGraphics::install(&ctx, config)` and passes that object to JavaScript.
An existing canvas can also call `await graphics.attach("preview", canvas)`. Call `remove()` when the application removes it.
The frame renderer handles resize and removal for a DrawingSurface control.

## Lend a browser device

Use `BrowserDeviceExchange::install` with the context and action manager of the application.
Register `GraphicsReady` in the action registry and pass `host:foundation_gpu_ready` to `install`.
To use a different action, register a host action with typed input `lease::Notice`.
In that action, get `DeviceExchange` from the context and call `accept(&event.input_value)`.
The graphics action stays local when the action manager also has a server transport.

A Rust action gets `DeviceExchange` from its context and calls `request(Selection { device, surface })`.
The request returns a ticket. Await its `wait()` method to obtain a lease or an error.
Call `lease.compute` for buffer work or `lease.process_texture` to change the supplied texture.
Drop the lease when Rust finishes its work.

Pass a JavaScript function to `install` to handle each request. For example, an editor can lend its existing device.

```js
function lend(request) {
  if (request.device !== "editor" || (request.surface && request.surface !== "preview")) {
    return request.deny().catch(reportGraphicsError);
  }
  const resource = request.surface ? previewTexture : undefined;
  pauseEditorGraphics();
  return request.ready(editorDevice, resource, generation, lost => {
    if (lost) rebuildEditorGraphics();
    else resumeEditorGraphics();
  }).catch(reportGraphicsError);
}
```

The generation is a positive JavaScript `BigInt`. Increase it after the host recreates the device or target.
The browser bridge keeps the supplied device and texture on their event loop.
The WebGPU backend in wgpu 30 exposes creation through an adapter. The bridge submits commands on the supplied device and queue.
The bridge checks the device identity by binding the supplied texture before it grants the lease.
Supply a single-sample two-dimensional `rgba8unorm` texture with `TEXTURE_BINDING` and `COPY_DST` usage.
For compute, omit the texture and the surface name.

Stop host access to the leased resource before calling `ready`.
The bridge waits for prior commands on the host queue. It invokes the return callback after the final queue fence or device loss.
The host owns the canvas and presentation. It can copy the result to the canvas after the return callback.
Call `withdraw()` on the returned grant to stop new work when the host removes the target.
The ticket returns an error for denial, cancellation, incompatible resources, a stale generation, or device loss.
A cancelled or rejected grant still returns its resources through the callback.

## Verification

Run `cargo test -p foundation_ui_gpu` for adapter-independent checks.
Run `cargo test -p foundation_ui_gpu -- --include-ignored` on a graphics-capable native machine.
The native tests cover compute, unsupported limits, device loss, reinitialization, and queued cancellation.
For Chrome, serve the repository root and run the following command. Supply the path to the Chrome executable.

```sh
cargo build --profile uat --target wasm32-unknown-unknown -p foundation_ui_gpu --example browser_handoff_test
mise exec -- wasm-bindgen --target web target/wasm32-unknown-unknown/uat/examples/browser_handoff_test.wasm --out-dir target/ui-gpu-web
node backends/foundation_ui_gpu/tests/browser/check.mjs "$CHROME_BIN" http://127.0.0.1:8000
```

The browser check reads back both compute paths and samples three targets from a screenshot.
It checks texture readback, device loss, and cancellation during import.
It also checks that the configured local action bypasses an attached server transport.
The Swift test reads both Metal textures. The Kotlin test uses Android PixelCopy to sample both native windows.

## Borrow a device from the host

A lease gives local Rust code temporary access to a device that the application already owns.
The application keeps its native handles in its provider. Rust receives a request number and the names of the resources.

Register `GraphicsReady` in the local action registry before installing the platform provider.
The built-in action reference is `host:foundation_gpu_ready`.
A custom local action that accepts `Notice` can call `DeviceExchange::accept` on the same context.
The provider must finish its previous use of the resources before it grants access.

Start the request from a local action. Return from the action so the host can dispatch its reply.
Wait for the result in a separate task.

```rust
use foundation_context::Ctx;
use foundation_ui_gpu::lease::{DeviceExchange, LeaseError, PendingLease, Selection};

fn request_preview(ctx: &Ctx) -> Result<PendingLease, LeaseError> {
    let exchange = ctx.get::<DeviceExchange>()
        .ok_or_else(|| LeaseError::Unavailable("No graphics provider".into()))?;
    exchange.request(Selection {
        device: "editor".into(),
        surface: Some("preview".into()),
    })
}

async fn update_preview(ticket: PendingLease, shader: String) -> Result<(), LeaseError> {
    let lease = ticket.wait().await?;
    lease.process_texture(shader, [1, 1, 1]).await?;
    drop(lease);
    Ok(())
}
```

For computation without a surface, use `surface: None` and call `lease.compute`.
Dropping a pending ticket cancels its request. A denied request returns `LeaseError::Denied`.
A replacement generation invalidates the old lease. A lost device requires a fresh host device.
The host resumes use only after its completion callback. The host owns final presentation and device destruction.
