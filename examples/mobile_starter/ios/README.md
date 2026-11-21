# Remote action starter

Open `Starter.xcodeproj` in Xcode. The app requests version 0.1.0 of the `EweUI` library from the `ewe_platform` repository.
Swift Package Manager checks the native archive checksum when it downloads the package.

The app reads `/native-starter` for its first frame. Refresh sends `server:refresh_count` to `/action`.
The server returns a replacement for the count view.

Set the server address in `App/App.swift`. Run the example server at port 8080 to use the default address.
The simulator uses the Mac loopback address. Use a reachable network address on an iPhone.

From the repository root, run `scripts/ui_native/release/test-ios-starter.sh` to verify a published version.
Pass `--local-package /path/to/candidate` to test a candidate before publication.
The test uses a temporary copy of the project. The checked-in project keeps its public package reference.
