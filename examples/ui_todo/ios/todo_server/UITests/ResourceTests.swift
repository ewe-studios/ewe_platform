import CryptoKit
import EweCore
import EweTodoCore
import EweUI
import XCTest

final class ResourceTests: XCTestCase {
    func testInstalledBundleReader() async throws {
        let environment = ProcessInfo.processInfo.environment
        try XCTSkipUnless(environment["EWE_RESOURCE_TEST_BUNDLE"] != nil,
                          "Run the installed package check with the application bundle path and expected hash")
        let path = try XCTUnwrap(environment["EWE_RESOURCE_TEST_BUNDLE"], "Pass the path of the installed TodoServer bundle")
        let bundle = try XCTUnwrap(Bundle(path: path))
        let core = try createLocalTodoCore(platform: .ios)
        do {
            _ = try await core.openResource(name: "art/todo.svg")
            XCTFail("Expected a missing host")
        } catch NativeResourceError.MissingHost { }
        core.installResources(source: BundleResourceSource(bundle: bundle))
        let file = try await core.openResource(name: "art/todo.svg")
        var hash = SHA256()
        while let bytes = try await file.nextChunk() {
            hash.update(data: bytes)
        }
        XCTAssertEqual(hash.finalize().map { String(format: "%02x", $0) }.joined(),
                       try XCTUnwrap(environment["EWE_RESOURCE_TEST_HASH"]))
        do {
            _ = try await core.openResource(name: "app.js")
            XCTFail("Unexpected browser-only file")
        } catch NativeResourceError.Missing(let name) { XCTAssertEqual(name, "app.js") }
        do {
            _ = try await core.openResource(name: "../outside")
            XCTFail("Expected an invalid resource name")
        } catch NativeResourceError.InvalidName { }
    }
}
