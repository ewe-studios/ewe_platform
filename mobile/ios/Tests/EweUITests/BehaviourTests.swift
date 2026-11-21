import EweWire
import FlatBuffers
import SwiftUI
import XCTest

@testable import EweUI

final class BehaviourTests: XCTestCase {
  @MainActor func testSharedBindingsAndSettings() async throws {
    let registry = BehaviourRegistry(motion: MotionSettings(duration: 0.25, dismissDistance: 45))
    XCTAssertEqual(registry.motion.duration, 0.25)
    XCTAssertEqual(registry.motion.dismissDistance, 45)
    for name in ["DIALOG", "DISMISS", "SCROLL_LOCK", "TRANSITION"] {
      XCTAssertTrue(registry.has(name))
    }
  }
  @MainActor func testUnknownBehaviourRetainsItsNumericID() async throws {
    var b = FlatBufferBuilder()
    let start = ewe_ui_UiBehaviour.startUiBehaviour(&b)
    ewe_ui_UiBehaviour.add(nodeId: 40, &b)
    // An old reader must report identifiers added by a newer sender.
    b.add(element: UInt16(999), def: UInt16(0), at: 6)
    let behaviour = ewe_ui_UiBehaviour.endUiBehaviour(&b, start: start)
    let op = ewe_ui_Operation.createOperation(&b, uiType: .uibehaviour, uiOffset: behaviour)
    let msg = ewe_ui_Message.createMessage(&b, operationOffset: op)
    let rows = b.createVector(ofOffsets: [msg])
    let body = ewe_ui_FrameMessage.createFrameMessage(&b, messagesVectorOffset: rows)
    let frame = ewe_ui_Frame.createFrame(
      &b, schemaVersion: 1, kindType: .framemessage, kindOffset: body)
    ewe_ui_Frame.finish(&b, end: frame)
    let store = NodeStore()
    let node = try XCTUnwrap(store.make(40, .stack, 0))
    XCTAssertTrue(store.receive(Data(b.sizedByteArray)))
    XCTAssertEqual(
      store.reports.last, Report(code: "UnknownBehaviour", row: 0, node: 40, name: "999"))
    XCTAssertTrue(node.behaviours.isEmpty)
  }
}
