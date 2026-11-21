import EweWire
import FlatBuffers
import Foundation
import XCTest

@testable import EweUI

final class ConformanceTests: XCTestCase {
  @MainActor func testWireFixtures() async throws {
    for name in [
      "native_tree", "native_columnar", "native_replace", "native_mixed", "native_behaviour",
      "native_presentation", "native_unknown_kind",
    ] {
      let store = NodeStore()
      store.behaviours.register("pull_to_refresh") { view, _ in view }
      let dir = try XCTUnwrap(Bundle.module.resourceURL?.appendingPathComponent("Fixtures"))
      if ["native_mixed", "native_replace", "native_behaviour", "native_presentation"].contains(name) {
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
