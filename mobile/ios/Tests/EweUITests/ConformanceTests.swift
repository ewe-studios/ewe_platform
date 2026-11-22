import EweWire
import FlatBuffers
import Foundation
import XCTest

@testable import EweUI

final class ConformanceTests: XCTestCase {
  @MainActor func testPortableTargets() throws {
    struct Step: Decodable { let bytes: [UInt8]; let mount: UInt32?; let error: String?; let `case`: String }
    let url = try XCTUnwrap(Bundle.module.url(forResource: "portable_targets", withExtension: "json", subdirectory: "Fixtures"))
    let steps = try JSONDecoder().decode([Step].self, from: Data(contentsOf: url))
    let store = NodeStore()
    try store.registerView("main", root: 1)
    try store.registerView("second", root: 7)
    for step in steps {
      if step.case == "custom" { try store.registerElement("save", tag: "InvoiceEditor", node: 42) }
      let count = store.reports.count
      XCTAssertTrue(store.receive(Data(step.bytes), at: step.mount), step.case)
      XCTAssertEqual(store.reports.dropFirst(count).map(\.code), step.error.map { [$0] } ?? [], step.case)
    }
    XCTAssertEqual(store.record(1)?.children, [44,48])
    XCTAssertEqual(store.record(7)?.children, [41,47])
    let countFrame = try XCTUnwrap(Bundle.module.url(forResource: "portable_count", withExtension: "bin", subdirectory: "Fixtures"))
    XCTAssertTrue(store.receive(try Data(contentsOf: countFrame), at: 1))
    let countNode = try XCTUnwrap(store.record(1)?.children.last)
    XCTAssertEqual(store.record(countNode)?.text, "1 open todo")
    let second = try XCTUnwrap(store.viewStore(named: "second"))
    try second.registerElement("open-count", tag: "ListFooter", node: 47)
    XCTAssertTrue(store.receive(try Data(contentsOf: countFrame), at: 7))
    XCTAssertEqual(second.record(7)?.children.last, countNode)
    XCTAssertEqual(second.record(countNode)?.text, "1 open todo")
    XCTAssertFalse(second.record(countNode) === store.record(countNode))
    try store.unregisterView("main")
    XCTAssertEqual(second.record(countNode)?.text, "1 open todo")
    XCTAssertNil(store.record(44))
    XCTAssertEqual(store.record(7)?.children, [41,countNode])
  }

  @MainActor func testReloadReplacesOnlyTheReceivingMount() async throws {
    let store = NodeStore()
    let dir = try XCTUnwrap(Bundle.module.resourceURL?.appendingPathComponent("Fixtures"))
    XCTAssertTrue(store.receive(try Data(contentsOf: dir.appendingPathComponent("native_tree.bin"))))
    let bytes = try Data(contentsOf: dir.appendingPathComponent("native_full_view.bin"))
    for _ in 0..<2 { XCTAssertTrue(store.receive(bytes, at: 41)) }
    XCTAssertEqual(store.record(41)?.children, [70])
    XCTAssertEqual(store.record(40)?.children, [41, 42])
    XCTAssertEqual(store.record(70)?.text, "Next view")
    XCTAssertTrue(store.reports.isEmpty)
  }

  @MainActor func testWireFixtures() async throws {
    for name in [
      "native_tree", "native_full_view", "native_columnar", "native_replace", "native_mixed", "native_behaviour",
      "native_presentation", "native_unknown_kind",
    ] {
      let store = NodeStore()
      store.behaviours.register("pull_to_refresh") { view, _ in view }
      let dir = try XCTUnwrap(Bundle.module.resourceURL?.appendingPathComponent("Fixtures"))
      if ["native_full_view", "native_mixed", "native_replace", "native_behaviour", "native_presentation"].contains(name) {
        XCTAssertTrue(
          store.receive(try Data(contentsOf: dir.appendingPathComponent("native_tree.bin"))))
      }
      let bytes = try Data(contentsOf: dir.appendingPathComponent(name + ".bin"))
      let expected =
        try JSONSerialization.jsonObject(
          with: Data(contentsOf: dir.appendingPathComponent(name + ".expected.json")))
        as? NSDictionary
      XCTAssertTrue(store.receive(bytes), name)
      let result = try JSONSerialization.jsonObject(with: store.snapshotJSON()) as? NSDictionary
      XCTAssertEqual(result, expected, name)
    }
  }
}

extension ConformanceTests {
  @MainActor func testCustomSignedStateSurvivesWireAndAction() async throws {
    let dir = try XCTUnwrap(Bundle.module.resourceURL?.appendingPathComponent("Fixtures"))
    let state = try Data(contentsOf: dir.appendingPathComponent("state_counter_custom.json"))
    let store = NodeStore()
    _ = store.make(42, .button, 0)
    XCTAssertTrue(
      store.receive(try Data(contentsOf: dir.appendingPathComponent("frame_state_custom.bin"))))
    XCTAssertEqual(store.record(42)?.state, state)
    var bytes = ByteBuffer(
      data: try Data(contentsOf: dir.appendingPathComponent("action_increment_custom.bin")))
    let frame: ewe_ui_Frame = try getCheckedRoot(byteBuffer: &bytes, fileId: "EWF1")
    let action = try XCTUnwrap(frame.kind(type: ewe_ui_FrameRequest.self)?.action)
    XCTAssertEqual(action.action, "server:increment_counter")
    XCTAssertEqual(action.nodeId, 42)
    XCTAssertEqual(action.input?.eventName, "tap")
    XCTAssertEqual(Data(action.state), state)
  }
  @MainActor func testAllWebRowsAreReported() async throws {
    let dir = try XCTUnwrap(Bundle.module.resourceURL?.appendingPathComponent("Fixtures"))
    let frameData = try Data(contentsOf: dir.appendingPathComponent("frame_ops_all_forms.bin"))
    var bytes = ByteBuffer(data: frameData)
    let frame: ewe_ui_Frame = try getCheckedRoot(byteBuffer: &bytes, fileId: "EWF1")
    let body = try XCTUnwrap(frame.kind(type: ewe_ui_FrameMessage.self))
    let count = (0..<body.messagesCount).filter {
      body.messages(at: $0)?.operation?.webType != .none_
    }.count
    let store = NodeStore()
    XCTAssertTrue(store.receive(frameData))
    XCTAssertGreaterThan(count, 0)
    XCTAssertEqual(store.reports.filter { $0.code == "UnsupportedPlatformOp" }.count, count)
  }
}
