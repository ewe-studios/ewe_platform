# Todo Server

This app loads its first frame from the Rust server. Task changes go to the server.
Open the add sheet and select filters on the device.

From the repository root, run these commands.

```sh
native-task task run native:todo:prepare-ios
cargo run --locked --profile uat -p ui_todo --bin ui_todo -- 127.0.0.1:8180
```

Open `TodoServer.xcodeproj` in Xcode. Select an iPhone simulator, then run the `TodoServer` scheme.
The installed app uses the address from the `EWE_TODO_SERVER_URL` build setting.
Set this value when building for a different server.
Set `EWE_TODO_URL` in the scheme’s Run environment to override the address for one launch.

For a physical iPhone, select your team under Signing & Capabilities.
Set `EWE_TODO_URL` to a server address that the iPhone can reach.
Select the connected iPhone as the run destination. Run the same scheme.

To test both iPhone apps on one simulator, run `native-task task run native:todo:test-ios SIMULATOR_ID` from the repository root.
Get the simulator identifier with `xcrun simctl list devices available`.
