import EweWire
import FlatBuffers
import Foundation
import Observation
import SwiftUI
import XCTest

@testable import EweUI

private func textFrame(_ sequence: UInt64, _ label: String, node: UInt32 = 42, ack: UInt64 = 0)
  -> Data
{
  var b = FlatBufferBuilder()
  let string = b.create(string: label)
  let text = ewe_ui_SetText.createSetText(&b, nodeId: node, textOffset: string)
  let op = ewe_ui_Operation.createOperation(&b, uiType: .settext, uiOffset: text)
  let message = ewe_ui_Message.createMessage(&b, operationOffset: op)
  let messages = b.createVector(ofOffsets: [message])
  let body = ewe_ui_FrameMessage.createFrameMessage(&b, ack: ack, messagesVectorOffset: messages)
  let frame = ewe_ui_Frame.createFrame(
    &b, schemaVersion: 1, streamId: 9, sequenceId: sequence, kindType: .framemessage,
    kindOffset: body)
  ewe_ui_Frame.finish(&b, end: frame)
  return Data(b.sizedByteArray)
}

@MainActor private final class CapturingCore: ActionCore {
  var events: [EventInput] = []
  var invoked: (() -> Void)?
  var fail = false
  func applyCapsFrame(_ frame: Data) throws {}
  func trigger(reference: String, event: EventInput) async throws -> Data? {
    events.append(event)
    invoked?()
    if fail { throw URLError(.badServerResponse) }
    return nil
  }
}

@MainActor private final class FakeChannel: Channel {
  var closed = false
  func close() { closed = true }
}
@MainActor private final class FakeChannels: ChannelFactory {
  var connections: [(UInt64, FakeChannel)] = []
  func open(
    endpoint: URL, transport: String, streamId: UInt64, resumeFrom: UInt64,
    onFrame: @escaping @MainActor (Data) -> Void, onError: @escaping @MainActor (String) -> Void
  ) -> any Channel {
    let channel = FakeChannel()
    connections.append((resumeFrom, channel))
    return channel
  }
}

final class RendererTests: XCTestCase {
  @MainActor func testOrderingAndObservation() async throws {
    let store = NodeStore()
    let a = try XCTUnwrap(store.make(42, .button, 0))
    let b = try XCTUnwrap(store.make(43, .text, 0))
    let observed = expectation(description: "updated node")
    let other = expectation(description: "unmodified node")
    other.isInverted = true
    withObservationTracking({ _ = a.text }, onChange: { observed.fulfill() })
    withObservationTracking({ _ = b.text }, onChange: { other.fulfill() })
    store.setCursor(stream: 9, sequence: 1)
    store.receive(textFrame(3, "three"))
    XCTAssertNil(a.text)
    store.receive(textFrame(20, "answer", ack: 5))
    XCTAssertEqual(a.text, "answer")
    store.receive(textFrame(2, "two"))
    XCTAssertEqual(a.text, "three")
    store.receive(textFrame(2, "replayed"))
    XCTAssertEqual(a.text, "three")
    await fulfillment(of: [observed, other], timeout: 0.05)
  }

  @MainActor func testGapReconnectsAndRemovalCloses() async throws {
    let channels = FakeChannels()
    let host = LiveMountHost(factory: channels)
    let store = NodeStore(live: host, sleep: { _ in await Task.yield() })
    let mount = try XCTUnwrap(store.make(50, .stack, 0))
    mount.live = LiveSpec(
      kind: "Stream", channelID: 9, transport: "Sse", endpoint: "/count", resumeFrom: 0)
    host.reconcile(store)
    XCTAssertTrue(channels.connections.isEmpty)
    store.insert(50, 1, 0)
    host.reconcile(store)
    let gap = expectation(description: "resume from cursor")
    store.setCursor(stream: 9, sequence: 7)
    store.onGap = { stream, cursor in
      XCTAssertEqual(stream, 9)
      XCTAssertEqual(cursor, 7)
      gap.fulfill()
    }
    store.receive(textFrame(9, "gap"))
    await fulfillment(of: [gap], timeout: 2)
    XCTAssertEqual(channels.connections.map { $0.0 }, [0, 7])
    XCTAssertTrue(channels.connections[0].1.closed)
    store.remove(50, 0)
    XCTAssertTrue(channels.connections[1].1.closed)
    let local = try XCTUnwrap(store.make(51, .stack, 0))
    local.live = LiveSpec(
      kind: "Stream", channelID: 10, transport: "Local", endpoint: "", resumeFrom: 0)
    store.insert(51, 1, 0)
    host.reconcile(store)
    XCTAssertEqual(channels.connections.count, 2)
  }

  @MainActor func testEventUsesStateOnControlAndReportsFailure() async throws {
    let store = NodeStore()
    let core = CapturingCore()
    let bridge = EventBridge(store: store, core: core)
    let parent = try XCTUnwrap(store.make(40, .stack, 0))
    let control = try XCTUnwrap(store.make(42, .button, 0))
    store.insert(42, 40, 0)
    parent.state = Data([99])
    control.state = Data([1, 2, 3])
    bridge.send(.tap, node: 42)
    XCTAssertTrue(core.events.isEmpty)
    control.actions[.tap] = "server:edit"
    let failed = expectation(description: "trigger error")
    core.fail = true
    store.onError = { code, _ in
      XCTAssertEqual(code, "ActionFailed")
      failed.fulfill()
    }
    bridge.send(.tap, node: 42, value: "hello", checked: true)
    await fulfillment(of: [failed], timeout: 2)
    let event = try XCTUnwrap(core.events.first)
    XCTAssertEqual(event.state, control.state)
    XCTAssertEqual(event.parentNode, 40)
    XCTAssertEqual(event.value, "hello")
    XCTAssertTrue(event.checked)
  }

  @MainActor func testMalformedFramesAndUnknownNode() async throws {
    let good = textFrame(0, "x")
    XCTAssertFalse(NodeStore().receive(Data(repeating: 255, count: 16)))
    for size in 0..<good.count {
      let s = NodeStore()
      _ = s.receive(good.prefix(size))
      XCTAssertFalse(s.reports.isEmpty)
    }
    let small = NodeStore(maxFrameBytes: 10)
    XCTAssertFalse(small.receive(good))
    XCTAssertEqual(small.reports.first?.code, "TooLarge")
    let s = NodeStore()
    XCTAssertTrue(s.receive(good))
    XCTAssertEqual(s.reports.first?.code, "UnknownNode")
  }

  #if os(macOS)
    @MainActor func testEveryCoreViewLaysOut() async throws {
      let store = NodeStore()
      let core = CapturingCore()
      let kinds: [NodeKind] = [
        .stack, .overlay, .scroll, .list, .text, .image, .button, .textInput, .toggle, .spacer,
        .unknown,
      ]
      for (index, kind) in kinds.enumerated() {
        let id = UInt32(80 + index)
        _ = store.make(id, kind, index)
        store.insert(id, 1, index)
      }
      let view = NSHostingView(
        rootView: EweScreen(store: store, bridge: EventBridge(store: store, core: core)))
      view.frame = CGRect(x: 0, y: 0, width: 320, height: 640)
      view.layoutSubtreeIfNeeded()
      XCTAssertGreaterThan(view.fittingSize.height, 0)
    }
  #endif
}
