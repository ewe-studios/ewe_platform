# Todo Server

The app loads its first frame from the Rust server. The server owns add, complete, reopen and delete actions.
The device opens forms and selects a filter. The library sends actions and applies frames.

[Build and install both apps](../README.md). Then start a fresh server from the repository root.

```sh
cargo run --profile uat -p ui_todo -- 127.0.0.1:8180
```

The project sets the default address with `eweTodoServerUrl` in `gradle.properties`.
Set this property with `-PeweTodoServerUrl=http://127.0.0.1:8180/` to use a forwarded address in the build.
From a second terminal, select the emulator and configure port forwarding.

```sh
adb reverse tcp:8180 tcp:8180
adb shell am start -n ewe.todo/.MainActivity --es server http://127.0.0.1:8180/
```

A valid `server` launch extra replaces the address and saves it in the app's private settings.
The launcher reuses the saved address. An invalid launch extra leaves the saved address in place.
The app uses the configured default when there is no saved address.

Tap **Add**, enter a title and tap **Add** in the sheet. Select **Complete**, **Done**, and **Reopen**.
Then select **Open**, **Delete**, and **Confirm**. A second client can read changes while the same Rust server runs.
