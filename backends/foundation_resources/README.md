# Application resources

A resource is a packaged file with a stable relative name.
The build script stages each resource before the host packages it.
Application code opens the same name in a browser, an Apple bundle, or an Android package.

## Declare the files

Put the declarations in the application package's `resources.toml`.

```toml
[[resource]]
name = "art"
path = "assets/art"

[[resource]]
name = "version.txt"
generated = "version.txt"

[[resource]]
name = "browser.js"
path = "web/browser.js"
targets = ["web"]
```

Use `path` for package-relative files and directories.
Use `generated` for files under Cargo's `OUT_DIR`.
The target names are `web`, `apple`, and `android`.
An entry without a target list applies to each target.

## Build the package

Add this library as a build dependency. Disable its default features and enable `build`.
Run each producer before staging its output.

```rust
fn main() -> std::io::Result<()> {
    // The application generates version.txt in OUT_DIR before this call.
    foundation_resources::build::stage_from_cargo()?;
    Ok(())
}
```

The helper supplies `EWE_STAGED_RESOURCES` to the compiler for release embedding.
Web development builds also stage `public/<package-name>` beside the server executable.
A native packager sets `EWE_RESOURCE_TARGET` and `EWE_RESOURCE_DEST` in the Cargo producer environment.
Use a destination specific to the application, target, and build configuration.
An Apple packager sets `EWE_RESOURCE_PAYLOAD` to its bundle folder name.
It copies only that folder. The staging index stays in the parent directory.
The Todo example registers its producer in both Xcode and Gradle.

## Read one resource

The application host installs a reader in its Foundation context.
Enable the default `runtime` feature to read through that context.

```rust,ignore
use foundation_resources::Resources;
use futures_util::StreamExt;

let mut icon = Resources::from_context(&ctx).open("art/icon.svg").await?;
while let Some(chunk) = icon.next().await {
    destination.write(chunk?).await?;
}
```

Invalid names, missing readers, absent files, and read failures have separate error types.
A read can fail after a successful open. Dropping the stream closes its file or browser body.

The browser application can use `BrowserResources` from the `browser-bindings` build.
It supplies the public base address and reads through `open(name)` and `nextChunk()`.
The Apple host installs `BundleResourceSource`. The Android host installs `AndroidResourceSource`.
