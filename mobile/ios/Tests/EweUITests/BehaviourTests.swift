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
  @MainActor func testBehaviourAppliesToOneNodeAndReportsUnknownRows() throws {
    let store = NodeStore()
    let selected = try XCTUnwrap(store.make(40, .stack, 0))
    let sibling = try XCTUnwrap(store.make(41, .stack, 0))
    store.insert(40, 1, 0)
    store.insert(41, 1, 0)

    var builder = FlatBufferBuilder()
    func shared(_ kind: ewe_ui_UiOperation, _ body: Offset) -> Offset {
      let op = ewe_ui_Operation.createOperation(&builder, uiType: kind, uiOffset: body)
      return ewe_ui_Message.createMessage(&builder, operationOffset: op)
    }

    let dismiss = ewe_ui_UiBehaviour.createUiBehaviour(&builder, nodeId: 40, behaviour: .dismiss)
    let known = shared(.uibehaviour, dismiss)
    let start = ewe_ui_UiBehaviour.startUiBehaviour(&builder)
    ewe_ui_UiBehaviour.add(nodeId: 40, &builder)
    builder.add(element: UInt16(999), def: UInt16(0), at: 6)
    let numeric = ewe_ui_UiBehaviour.endUiBehaviour(&builder, start: start)
    let unknownShared = shared(.uibehaviour, numeric)
    let name = builder.create(string: "fixture_unregistered")
    let custom = ewe_ui_MobileBehaviour.createMobileBehaviour(&builder, nodeId: 40, customOffset: name)
    let mobile = ewe_ui_Operation.createOperation(&builder, mobileType: .mobilebehaviour, mobileOffset: custom)
    let unknownCustom = ewe_ui_Message.createMessage(&builder, operationOffset: mobile)
    let label = builder.create(string: "Still visible")
    let text = ewe_ui_SetText.createSetText(&builder, nodeId: 40, textOffset: label)
    let validText = shared(.settext, text)
    let rows = builder.createVector(ofOffsets: [known, unknownShared, unknownCustom, validText])
    let body = ewe_ui_FrameMessage.createFrameMessage(&builder, messagesVectorOffset: rows)
    let frame = ewe_ui_Frame.createFrame(&builder, schemaVersion: 1, kindType: .framemessage, kindOffset: body)
    ewe_ui_Frame.finish(&builder, end: frame)

    XCTAssertTrue(store.receive(Data(builder.sizedByteArray)))
    XCTAssertEqual(selected.behaviours, ["DISMISS"])
    XCTAssertTrue(sibling.behaviours.isEmpty)
    XCTAssertEqual(selected.text, "Still visible")
    XCTAssertNil(sibling.text)
    XCTAssertEqual(store.reports, [
      Report(code: "UnknownBehaviour", row: 1, node: 40, name: "999"),
      Report(code: "UnknownBehaviour", row: 2, node: 40, name: "fixture_unregistered"),
    ])

    store.remove(40, 0)
    XCTAssertNil(store.record(40))
    XCTAssertTrue(store.record(41) === sibling)
    let replacement = try XCTUnwrap(store.make(40, .stack, 0))
    store.insert(40, 1, 0)
    XCTAssertFalse(replacement === selected)
    XCTAssertTrue(replacement.behaviours.isEmpty)
    XCTAssertTrue(store.receive(Data(builder.sizedByteArray)))
    XCTAssertEqual(replacement.behaviours, ["DISMISS"])
    XCTAssertTrue(sibling.behaviours.isEmpty)
  }

}
