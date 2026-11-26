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
Enable `browser-package` for a server build that produces browser bindings and JavaScript.
Declare generated input names in `resources.toml`. Supply their contents to the same hook.

```rust
fn main() -> std::io::Result<()> {
    foundation_resources::package::BrowserPackage::new(std::path::Path::new("web/app.js"))
        .browser_enabled(cfg!(feature = "server"))
        .with_connect()
        .with_resources()
        .with_generated("version.txt", env!("CARGO_PKG_VERSION"))
        .build()
}
```

The default application binary is `module.wasm`. Set `with_module_path("assets/site.wasm")` to change its public name.
Import `moduleUrl` from `./ewe-app-config.js` in the application entry. It resolves relative to the loaded application script.

Use `with_companion(feature, directory)` for another WebAssembly target from the application crate.
Import its generated JavaScript from that directory. A dynamic import delays binary initialization until the application needs it.
Resource-only consumers can call `foundation_resources::build::stage_from_cargo()` after running their own producers.

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

### Native launch assets

Enable the `mobile-launch` build feature to compile application artwork.
The project supplies `icon.png`, `splash.png`, and `launch-theme.toml` in its public directory.
Use a square, opaque launcher icon. The splash image can contain transparent pixels.

```toml
[background]
light = "#F8FAFC"
dark = "#101820"
```

The application build script calls `build::launch::stage_from_cargo(public)`.
The host build sets `EWE_LAUNCH_DEST` and selects `android` or `apple` through `EWE_RESOURCE_TARGET`.
Android compiles the generated `res` directory. Apple compiles `Launch.xcassets`.
Cargo tracks each source file. A failed build preserves the previous native resource set.
