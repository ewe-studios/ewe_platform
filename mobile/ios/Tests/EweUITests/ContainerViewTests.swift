import SwiftUI
import XCTest
@testable import EweUI

@MainActor private final class ContainerTestCore: ActionCore {
  var events = [EventInput]()
  var onEvent: (() -> Void)?
  func applyCapsFrame(_ frame: Data) throws {}
  func trigger(reference: String, event: EventInput) async throws -> Data? {
    events.append(event)
    onEvent?()
    return nil
  }
}

#if os(iOS)
  private struct HitProbe: UIViewRepresentable {
    let view: UIView
    func makeUIView(context: Context) -> UIView { view }
    func updateUIView(_ view: UIView, context: Context) {}
  }
#endif

final class ContainerViewTests: XCTestCase {
  @MainActor func testModalFindsOnlyItsOwnVisibleCancelAction() throws {
    let store = NodeStore()
    let screen = EweScreen(store: store, bridge: EventBridge(store: store, core: ContainerTestCore()))
    let modal = try XCTUnwrap(store.make(10, .modal, 0))
    let nested = try XCTUnwrap(store.make(11, .modal, 0))
    let panel = try XCTUnwrap(store.make(12, .bottomSheet, 0))
    store.insert(nested.id, modal.id, 0)
    store.insert(panel.id, modal.id, 0)

    let nestedCancel = try XCTUnwrap(store.make(13, .cancelButton, 0))
    let cancel = try XCTUnwrap(store.make(14, .cancelButton, 0))
    for node in [nestedCancel, cancel] {
      node.actions[.tap] = "host:close"
    }
    store.insert(nestedCancel.id, nested.id, 0)
    store.insert(cancel.id, panel.id, 0)
    let component = EweModal(screen: screen, node: modal) { EmptyView() }
    XCTAssertIdentical(component.cancelControl(in: modal), cancel)
    cancel.props[.hidden] = .bool(true)
    XCTAssertNil(component.cancelControl(in: modal))
  }

  @MainActor func testSubmitKeepsTheFieldIdentityAndDraft() throws {
    let store = NodeStore()
    let screen = EweScreen(store: store, bridge: EventBridge(store: store, core: ContainerTestCore()))
    let panel = try XCTUnwrap(store.make(10, .bottomSheet, 0))
    let field = try XCTUnwrap(store.make(11, .textInput, 0))
    let submit = try XCTUnwrap(store.make(12, .submitButton, 0))
    field.actions[.submit] = "server:add"
    store.insert(field.id, panel.id, 0)
    store.insert(submit.id, panel.id, 0)
    let draft = SheetDraft()
    draft.set("Milk", for: field)
    XCTAssertIdentical(screen.submitField(for: submit), field)
    XCTAssertEqual(draft.value(for: field), "Milk")
    field.props[.enabled] = .bool(false)
    XCTAssertNil(screen.submitField(for: submit))
    store.remove(field.id, 0)
    let replacement = try XCTUnwrap(store.make(field.id, .textInput, 0))
    XCTAssertEqual(draft.value(for: replacement), "")
  }

  @MainActor func testSubmitButtonSendsOneEventWithTheFieldState() async throws {
    let store = NodeStore()
    let core = ContainerTestCore()
    let screen = EweScreen(store: store, bridge: EventBridge(store: store, core: core))
    let panel = try XCTUnwrap(store.make(10, .bottomSheet, 0))
    let field = try XCTUnwrap(store.make(11, .textInput, 0))
    let button = try XCTUnwrap(store.make(12, .submitButton, 0))
    store.insert(field.id, panel.id, 0)
    store.insert(button.id, panel.id, 0)
    field.actions[.submit] = "server:add"
    field.state = Data([7])
    let draft = SheetDraft()
    draft.set("Milk", for: field)

    let sent = expectation(description: "one submit event")
    core.onEvent = { sent.fulfill() }
    screen.sendButton(button, draft: draft)
    await fulfillment(of: [sent], timeout: 2)
    XCTAssertEqual(core.events.count, 1)
    let event = try XCTUnwrap(core.events.first)
    XCTAssertEqual(event.value, "Milk")
    XCTAssertEqual(event.nodeId, field.id)
    XCTAssertEqual(event.state, field.state)
  }

  @MainActor func testStructuredEventUsesTheTriggeringState() async throws {
    struct Input: Codable, Equatable { let title: String; let list: [String: Int] }
    let store = NodeStore()
    let core = ContainerTestCore()
    let bridge = EventBridge(store: store, core: core)
    let field = try XCTUnwrap(store.make(11, .textInput, 0))
    field.actions[.submit] = "server:add"
    field.state = Data([42])
    let received = expectation(description: "structured event")
    core.onEvent = { received.fulfill() }
    let value = Input(title: "Milk", list: ["id": 7])
    try bridge.sendStructured(.submit, node: field.id, value: value)
    await fulfillment(of: [received], timeout: 2)
    let input = try XCTUnwrap(core.events.first)
    XCTAssertEqual(input.state, field.state)
    XCTAssertEqual(input.nodeId, field.id)
    XCTAssertEqual(try JSONDecoder().decode(Input.self, from: XCTUnwrap(input.json)), value)
  }

  @MainActor func testStackPinsItsTrailingSheetToTheBottom() throws {
    let store = NodeStore()
    let screen = EweScreen(store: store,
      bridge: EventBridge(store: store, core: ContainerTestCore()), colors: [7: .red])
    let title = try XCTUnwrap(store.make(10, .text, 0))
    title.text = "Items"
    let sheet = try XCTUnwrap(store.make(11, .bottomSheet, 0))
    let sample = try XCTUnwrap(store.make(12, .stack, 0))
    sample.props[.background] = .color(7)
    sample.props[.height] = .length(12, "pt")
    sample.props[.width] = .length(40, "pt")
    store.insert(title.id, 1, 0)
    store.insert(sheet.id, 1, 0)
    store.insert(sample.id, sheet.id, 0)

    let renderer = ImageRenderer(content: screen.frame(width: 120, height: 160))
    let image = try XCTUnwrap(renderer.cgImage)
    let stride = image.width * 4
    let context = try XCTUnwrap(CGContext(data: nil, width: image.width, height: image.height,
      bitsPerComponent: 8, bytesPerRow: stride, space: CGColorSpaceCreateDeviceRGB(),
      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
    let red = try withExtendedLifetime(context) {
      let pixels = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
      return (0..<image.height).filter { y in
        let offset = y * stride + 24 * 4
        return pixels[offset] > 180 && pixels[offset + 1] < 90 && pixels[offset + 2] < 90
      }
    }
    XCTAssertFalse(red.isEmpty)
    XCTAssertGreaterThan(try XCTUnwrap(red.min()), 120)
  }

  #if os(iOS)
    @MainActor func testThemeChangePreservesTheEditedField() async throws {
      let store = NodeStore()
      let core = ContainerTestCore()
      let bridge = EventBridge(store: store, core: core)
      let input = try XCTUnwrap(store.make(32, .textInput, 0))
      input.props[.utilityClasses] = .text("p-md ios-p-lg web-only")
      store.insert(input.id, 1, 0)
      let draft = SheetDraft()
      func screen(_ theme: EweTheme) -> some View {
        EweScreen(store: store, bridge: bridge, theme: theme).environment(\.eweSheetDraft, draft)
      }
      let host = UIHostingController(rootView: screen(EweTheme()))
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 640))
      window.rootViewController = host
      window.makeKeyAndVisible()
      defer { window.isHidden = true }
      func textField(_ view: UIView) -> UITextField? {
        if let field = view as? UITextField { return field }
        return view.subviews.lazy.compactMap(textField).first
      }
      host.view.layoutIfNeeded()
      let field = try XCTUnwrap(textField(host.view))
      XCTAssertTrue(field.becomeFirstResponder())
      draft.set("Buy milk", for: input)
      host.view.layoutIfNeeded()
      await Task.yield()
      XCTAssertEqual(field.text, "Buy milk")
      let json = Data(#"{"p-lg":{"padding":["16px","16px"]}}"#.utf8)
      host.rootView = screen(try EweTheme(manifest: json))
      host.view.layoutIfNeeded()
      let after = try XCTUnwrap(textField(host.view))
      XCTAssertIdentical(field, after)
      XCTAssertTrue(after.isFirstResponder)
      XCTAssertEqual(after.text, "Buy milk")
      XCTAssertTrue(core.events.isEmpty)
    }

    @MainActor func testBottomSheetMeasuresTheSuppliedChildHeight() {
      func measured(_ height: CGFloat) -> CGFloat {
        let host = UIHostingController(rootView: EweBottomSheet {
          Color.red.frame(width: 80, height: height)
        })
        return host.sizeThatFits(in: CGSize(width: 320, height: 700)).height
      }
      XCTAssertEqual(measured(120) - measured(40), 80, accuracy: 1)
    }

    @MainActor func testOverlayDisplaysItsChildrenOnTheCurrentSurface() async {
      let identity = NSObject()
      let appeared = expectation(description: "overlay content appears")
      let background = UIView()
      let foreground = UIView()
      let root = HitProbe(view: background).frame(maxWidth: .infinity, maxHeight: .infinity)
        .background {
          EweOverlay(identity: ObjectIdentifier(identity)) {
            EweBottomSheet {
              HitProbe(view: foreground).frame(width: 60, height: 24)
                .onAppear { appeared.fulfill() }
            }
          }
        }
        .eweLayerHost()
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 640))
      let host = UIHostingController(rootView: root)
      window.rootViewController = host
      window.isHidden = false
      defer { window.isHidden = true }
      host.view.layoutIfNeeded()
      await fulfillment(of: [appeared], timeout: 3)
      host.view.layoutIfNeeded()
      let outside = background.convert(CGPoint(x: 10, y: 10), to: host.view)
      let inside = foreground.convert(CGPoint(x: 20, y: 10), to: host.view)
      XCTAssertIdentical(host.view.hitTest(outside, with: nil), background)
      XCTAssertIdentical(host.view.hitTest(inside, with: nil), foreground)
    }

    @MainActor func testOverlayDoesNotAddChildHeightToParent() {
      let id = NSObject()
      let host = UIHostingController(rootView:
        VStack(spacing: 0) {
          Color.blue.frame(width: 200, height: 100)
          EweOverlay(identity: ObjectIdentifier(id)) {
            EweBottomSheet { Text("A floating panel") }
          }
        }.eweLayerHost())
      XCTAssertEqual(host.sizeThatFits(in: CGSize(width: 320, height: 700)).height, 100, accuracy: 1)
    }
  #endif
}
