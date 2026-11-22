# {{ PROJECT_NAME }}

Install Java 17 and the Android development kit. Use an Android emulator or connect a device.
{% if HOST == "on-device" %}
Install the Android Native Development Kit (NDK) and set `ANDROID_NDK_HOME` to its directory.
Install Rust and build the application library.

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk --locked
./build-rust.sh
```

The application stores its count on the device. Change the `increment` action in `rust/src/lib.rs`.
Run the script again after each Rust change.
{% else %}
To run the reference server, check out the matching Ewe repository and run these commands there.

```sh
examples/ui_todo/build-module.sh
cargo run -p ui_todo --bin ui_todo -- 127.0.0.1:8180
```

Change the server address with the `server` launch option.
The emulator reaches the development computer at `10.0.2.2`.
The screen loads from `GET /open-count`. Tap **Refresh** to send `server:refresh_count`.
Change the route and action in `MainActivity.kt`.
{% endif %}
Build and install the application. Start the activity with the Android Debug Bridge (adb).

```sh
./gradlew :app:installDebug
adb shell am start -n {{ APP_ID }}/.MainActivity{% if HOST == "server" %} --es server http://10.0.2.2:8180/{% endif %}
```
{% if HOST == "on-device" %}
Tap **Increment**. The count increases without a network connection.
{% endif %}
The application imports `{{ MAVEN_GROUP }}:ewe-ui:{{ VERSION }}`. The package renders the controls.
