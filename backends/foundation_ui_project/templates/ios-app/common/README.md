# {{ PROJECT_NAME }}

Open `{{ PROJECT_NAME }}.xcodeproj` in Xcode. Select an iPhone simulator and the `{{ PROJECT_NAME }}` scheme.
{% if HOST == "on-device" %}
Install Rust and Xcode. Add the Rust targets before the first build.
On an Intel Mac, use `x86_64-apple-ios` for the simulator.

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
{% if COMPOSED %}cd ..
ewe_ui build --target ios{% else %}./build-rust.sh{% endif %}
```

Run the scheme. Tap **Increment**. The count increases on the device.
Change the `increment` action in {% if COMPOSED %}`../native-core/src/lib.rs`{% else %}`rust/src/lib.rs`{% endif %}. Run the script again after each Rust change.
{% else %}
Set `EWE_SERVER` in the scheme environment. The default address is `http://127.0.0.1:8080`.
Run the scheme. The screen loads from `GET /open-count`. Tap **Refresh** to send `server:refresh_count` to the server.
Change the initial route and action in `App/App.swift`.

To run the reference server, check out the matching Ewe repository and run the following commands there.

```sh
cargo run -p ui_todo --bin ui_todo -- 127.0.0.1:8080
```
{% endif %}
Build from a shell with Xcode selected.

```sh
xcodebuild -project {{ PROJECT_NAME }}.xcodeproj -scheme {{ PROJECT_NAME }} -destination 'generic/platform=iOS Simulator' build
```

The application imports EweUI {{ VERSION }}. EweUI renders the controls.

## Application resources

Declare shared files in `{{ RESOURCE_PACKAGE }}/resources.toml`.
The project build runs Cargo before packaging the bundle or application assets.
Install Rust and Cargo before the first project build.
The sample includes a shared title, a generated version, and a browser-only script.
Use the same logical name when reading a file on each platform.
