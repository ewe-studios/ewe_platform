import EweWire
import FlatBuffers
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
@MainActor private final class CapturingFactory: ChannelFactory {
  var opened: [(stream: UInt64, cursor: UInt64, channel: CapturingChannel)] = []
  func open(
    endpoint: URL, transport: String, streamId: UInt64, resumeFrom: UInt64,
    onFrame: @escaping @MainActor (Data) -> Void, onError: @escaping @MainActor (String) -> Void
  ) -> any Channel {
    let channel = CapturingChannel()
    opened.append((streamId, resumeFrom, channel))
    return channel
  }
}

private func replayCapabilities() -> Data {
  var builder = FlatBufferBuilder()
  let caps = ewe_ui_ServerCaps.createServerCaps(
    &builder, schemaVersion: 1, vocabularyVersion: 1, streamId: 9, subscriptionResume: true)
  let control = ewe_ui_Control.createControl(&builder, capsOffset: caps)
  let message = ewe_ui_Message.createMessage(&builder, controlOffset: control)
  let messages = builder.createVector(ofOffsets: [message])
  let body = ewe_ui_FrameMessage.createFrameMessage(&builder, messagesVectorOffset: messages)
  let frame = ewe_ui_Frame.createFrame(
    &builder, schemaVersion: 1, streamId: 9, kindType: .framemessage, kindOffset: body)
  ewe_ui_Frame.finish(&builder, end: frame)
  return Data(builder.sizedByteArray)
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
  @MainActor func testDedicatedMountUsesItsOwnConnectionDuringReplay() async throws {
    let factory = CapturingFactory()
    let adapter = CapturingAdapter()
    let manager = ConnectionManager(adapters: ["websocket": adapter])
    let host = LiveMountHost(factory: factory, connections: manager)
    let store = NodeStore(live: host)
    defer { host.close(); manager.close() }
    XCTAssertTrue(store.receive(replayCapabilities()))
    XCTAssertTrue(store.subscriptionReplay)

    func attach(_ id: UInt32, stream: UInt64, cursor: UInt64, shared: Bool?, keepAlive: Bool = false) throws {
      let record = try XCTUnwrap(store.make(id, .stack, 0))
      record.live = LiveSpec(
        kind: "Stream", channelID: stream, transport: "WebSocket", endpoint: "wss://example.test/live",
        resumeFrom: cursor, shared: shared, keepAlive: keepAlive)
      store.insert(id, 1, 0)
      host.reconcile(store)
    }

    try attach(50, stream: 9, cursor: 23, shared: false)
    try attach(51, stream: 10, cursor: 51, shared: false)
    XCTAssertEqual(factory.opened.map(\.stream), [9, 10])
    XCTAssertEqual(factory.opened.map(\.cursor), [23, 51])
    XCTAssertTrue(adapter.opened.isEmpty)

    host.reconnect(stream: 9, cursor: 24, store: store)
    XCTAssertEqual(factory.opened.map(\.stream), [9, 10, 9])
    XCTAssertEqual(factory.opened.map(\.cursor), [23, 51, 24])
    XCTAssertEqual(factory.opened.map { $0.channel.closed }, [true, false, false])

    try attach(52, stream: 11, cursor: 0, shared: nil)
    try attach(53, stream: 12, cursor: 0, shared: true)
    XCTAssertEqual(adapter.opened.count, 1)
    try attach(54, stream: 13, cursor: 0, shared: false, keepAlive: true)
    try attach(55, stream: 14, cursor: 0, shared: false, keepAlive: true)
    XCTAssertEqual(adapter.opened.count, 3)
    store.remove(54, 0)
    store.remove(55, 0)
    XCTAssertEqual(adapter.opened.map(\.closed), [false, false, false])

    host.close()
    XCTAssertEqual(factory.opened.map { $0.channel.closed }, [true, true, true])
    XCTAssertEqual(adapter.opened.map(\.closed), [true, false, false])
    manager.close()
    XCTAssertEqual(adapter.opened.map(\.closed), [true, true, true])
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
