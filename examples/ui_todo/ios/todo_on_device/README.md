# Todo On Device

This app keeps its task store in the Rust core inside the app. The store lasts until the app process stops.
The app opens its first screen without a server. All actions run in the same local core.

From the repository root, run `native-task task run native:todo:prepare-ios`.
Open `TodoOnDevice.xcodeproj` in Xcode. Select an iPhone simulator, then run the `TodoOnDevice` scheme.

For a physical iPhone, select your team under Signing & Capabilities.
Select the connected iPhone as the run destination. Run the same scheme.

To test both iPhone apps on one simulator, run `native-task task run native:todo:test-ios SIMULATOR_ID` from the repository root.
Get the simulator identifier with `xcrun simctl list devices available`.

If the Rust bridge changes, run `native-task task run native:todo:prepare-ios --sync-bridge` to refresh its Swift bindings.
