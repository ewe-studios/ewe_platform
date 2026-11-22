import Foundation
import XCTest
@testable import EweUI

@MainActor private final class CapturingChannel: Channel {
  var closed = false
  func close() { closed = true }
}
@MainActor private final class CapturingAdapter: ConnectionAdapter {
  var opened: [CapturingChannel] = []
  var events: [ConnectionEvents] = []
  func open(endpoint: URL, events: ConnectionEvents) throws -> any Channel {
    let channel = CapturingChannel()
    opened.append(channel)
    self.events.append(events)
    return channel
  }
}
final class ConnectionManagerTests: XCTestCase {
  @MainActor func testSenderKeepsSocketAndFanoutKeepsBytesIndependent() async throws {
    let adapter = CapturingAdapter()
    let manager = ConnectionManager(adapters: ["websocket":adapter])
    let url = try XCTUnwrap(URL(string: "wss://example.test/live"))
    let first = try manager.acquire("websocket", endpoint: url, onError: { XCTFail($0) }) { data in
      var copy = data
      copy[0] = 0
    }
    var received = Data()
    let second = try manager.acquire("websocket", endpoint: url, onError: { XCTFail($0) }) { received = $0 }
    let sender = try manager.acquire("websocket", endpoint: url, onError: { XCTFail($0) })
    XCTAssertEqual(adapter.opened.count, 1)
    adapter.events[0].deliver(Data([8]))
    XCTAssertEqual(received, Data([8]))
    first.close(); second.close()
    XCTAssertFalse(adapter.events[0].listening())
    XCTAssertFalse(adapter.opened[0].closed)
    sender.close()
    XCTAssertTrue(adapter.opened[0].closed)
  }
  @MainActor func testEventSharingAndRetention() async throws {
    let adapter = CapturingAdapter()
    let manager = ConnectionManager(adapters: ["sse":adapter])
    let url = try XCTUnwrap(URL(string: "https://example.test/live"))
    func handle(_ shared: Bool? = nil, keep: Bool = false) throws -> any ConnectionHandle {
      try manager.acquire("sse", endpoint: url, shared: shared, keepAlive: keep, onError: { XCTFail($0) })
    }
    try handle().close()
    try handle().close()
    let held = try handle(true, keep: true)
    try handle(true).close()
    held.close()
    XCTAssertEqual(adapter.opened.count, 3)
    XCTAssertFalse(adapter.opened[2].closed)
    try handle(true).close()
    held.releaseRetention()
    XCTAssertTrue(adapter.opened[2].closed)
  }
}
