// swift-tools-version: 6.0
import Foundation
import PackageDescription

let localBinary = ".build-inputs/EweCoreFFI.xcframework"
let directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let core: Target = FileManager.default.fileExists(atPath: directory.appending(path: localBinary).path)
    ? .binaryTarget(name: "EweCoreFFI", path: localBinary)
    : .binaryTarget(name: "EweCoreFFI",
        url: "https://github.com/ewe-studios/ewe_platform/releases/download/native-ui-v0.1.0/EweCoreFFI.xcframework.zip",
        checksum: "1c339daa36ebb995d62dbd611b9fd423a40bfc8bd548fdae6cf26d4494948aa6")

let package = Package(name: "EweUI", platforms: [.iOS(.v17), .macOS(.v14)],
    products: [.library(name: "EweUI", targets: ["EweUI"])],
    dependencies: [.package(url: "https://github.com/google/flatbuffers.git", exact: "25.9.23")],
    targets: [
        core,
        .target(name: "EweGraphicsBridge", path: "mobile/ios/Sources/EweGraphicsBridge"),
        .target(name: "EweCore", dependencies: ["EweCoreFFI"], path: "mobile/ios/Sources/EweCore"),
        .target(name: "EweWire", dependencies: [.product(name: "FlatBuffers", package: "flatbuffers")], path: "mobile/ios/Sources/EweWire"),
        .target(name: "EweUI", dependencies: ["EweGraphicsBridge", "EweWire", .product(name: "FlatBuffers", package: "flatbuffers"),
            .target(name: "EweCore", condition: .when(platforms: [.iOS, .macOS]))], path: "mobile/ios/Sources/EweUI"),
        .testTarget(name: "EweUITests", dependencies: ["EweUI"], path: "mobile/ios/Tests/EweUITests", resources: [.copy("Fixtures")])
    ], swiftLanguageModes: [.v5])
