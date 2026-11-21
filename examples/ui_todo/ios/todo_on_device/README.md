# Todo On Device

This app keeps its task store in the Rust core inside the app. The store lasts until the app process stops.
The app opens its first screen without a server. All actions run in the same local core.

From the repository root, run `scripts/ui_native/prepare-ios-todo.sh`.
Open `TodoOnDevice.xcodeproj` in Xcode. Select an iPhone simulator, then run the `TodoOnDevice` scheme.

For a physical iPhone, select your team under Signing & Capabilities.
Select the connected iPhone as the run destination. Run the same scheme.

To test both iPhone apps on one simulator, run `scripts/ui_native/test-ios-todo.sh SIMULATOR_ID` from the repository root.
Get the simulator identifier with `xcrun simctl list devices available`.

If the Rust bridge changes, run `scripts/ui_native/prepare-ios-todo.sh --sync-bridge` to refresh its Swift bindings.
