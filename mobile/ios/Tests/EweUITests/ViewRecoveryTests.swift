import Foundation
import XCTest
@testable import EweUI

final class ViewRecoveryTests: XCTestCase {
  private func bytes(_ name: String) throws -> Data {
    let url = try XCTUnwrap(Bundle.module.url(forResource:name,withExtension:"bin",subdirectory:"Fixtures"))
    return try Data(contentsOf:url)
  }

  @MainActor func testFailureLeavesContentAndRetryUsesTheNewCursor() async throws {
    let store = NodeStore()
    var calls = 0
    let loader = store.registerLoader(maxAttempts:1) {
      calls += 1
      return try self.bytes(calls < 3 ? "native_full_view" : "recovery_snapshot")
    }
    try await loader.open()
    let first = loader.generation
    XCTAssertTrue(loader.accepts(first))
    do { try await loader.expire(stream:1); XCTFail("Missing cursor accepted") }
    catch ViewRecoveryError.missingCursor { }
    XCTAssertEqual(store.record(70)?.text,"Next view")
    XCTAssertFalse(loader.accepts(first))
    do { try await loader.expire(stream:1); XCTFail("Limit ignored") }
    catch ViewRecoveryError.attemptLimit { }
    XCTAssertEqual(calls,2)
    try await loader.retry()
    XCTAssertEqual(store.record(71)?.text,"Through twenty")
    XCTAssertEqual(store.cursor(stream:1),20)
    loader.close()
    XCTAssertFalse(loader.accepts(loader.generation))
  }

  @MainActor func testConcurrentExpiryCombinesLoadsAndDisposalIgnoresTheResult() async throws {
    let store = NodeStore()
    let started = expectation(description:"Loader started")
    var release: CheckedContinuation<Void,Never>?
    var calls = 0
    let loader = store.registerLoader {
      calls += 1
      await withCheckedContinuation { release = $0; started.fulfill() }
      return try self.bytes("recovery_snapshot")
    }
    let a = Task { try await loader.expire(stream:1) }
    await fulfillment(of:[started],timeout:5)
    let b = Task { try await loader.expire(stream:1) }
    await Task.yield()
    loader.close()
    release?.resume()
    for task in [a,b] {
      do { try await task.value; XCTFail("Disposed load installed") } catch { }
    }
    XCTAssertEqual(calls,1)
    XCTAssertNil(store.record(71))
  }
}
