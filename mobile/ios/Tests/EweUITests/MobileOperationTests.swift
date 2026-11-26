import EweWire
import FlatBuffers
import XCTest

@testable import EweUI

@MainActor private final class RecordingCommands: CommandHost {
  var seen: [ewe_ui_NativeCommandId] = []
  func run(_ command: ewe_ui_NativeCommandId, args: [String: Value]) -> Bool {
    seen.append(command)
    return true
  }
}

final class MobileOperationTests: XCTestCase {
  @MainActor func testNativeCommandsWebViewAndCustomBehaviour() async throws {
    var b = FlatBufferBuilder()
    var messages: [Offset] = []
    func row(_ kind: ewe_ui_MobileOperation, _ body: Offset) {
      let op = ewe_ui_Operation.createOperation(&b, mobileType: kind, mobileOffset: body)
      messages.append(ewe_ui_Message.createMessage(&b, operationOffset: op))
    }
    let haptic = ewe_ui_NativeCommand.createNativeCommand(&b, command: .haptic)
    row(.nativecommand, haptic)
    let unknown = ewe_ui_NativeCommand.createNativeCommand(&b)
    row(.nativecommand, unknown)
    let url = b.create(string: "https://example.com/view")
    let web = ewe_ui_MountWebView.createMountWebView(&b, nodeId: 70, parentId: 1, urlOffset: url)
    row(.mountwebview, web)
    let name = b.create(string: "pull_to_refresh")
    let custom = ewe_ui_MobileBehaviour.createMobileBehaviour(&b, nodeId: 70, customOffset: name)
    row(.mobilebehaviour, custom)
    let rows = b.createVector(ofOffsets: messages)
    let body = ewe_ui_FrameMessage.createFrameMessage(&b, messagesVectorOffset: rows)
    let frame = ewe_ui_Frame.createFrame(
      &b, schemaVersion: 1, kindType: .framemessage, kindOffset: body)
    ewe_ui_Frame.finish(&b, end: frame)
    let host = RecordingCommands()
    let store = NodeStore(commands: host)
    let node = try XCTUnwrap(store.make(70, .stack, 0))
    store.behaviours.register("pull_to_refresh") { view, _ in view }
    XCTAssertTrue(store.receive(Data(b.sizedByteArray)))
    XCTAssertEqual(host.seen, [.haptic])
    XCTAssertEqual(store.reports, [Report(code: "UnknownCommand", row: 1)])
    XCTAssertEqual(node.webViewURL, "https://example.com/view")
    XCTAssertEqual(node.webViewParent, 1)
    XCTAssertEqual(node.behaviours, ["pull_to_refresh"])
  }
}
