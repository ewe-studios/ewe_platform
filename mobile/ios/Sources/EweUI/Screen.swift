import EweWire
import SwiftUI

@MainActor public struct EweScreen: View {
  public let store: NodeStore
  public let bridge: EventBridge
  let theme: EweTheme
  public init(
    store: NodeStore, bridge: EventBridge, colors: [UInt32: Color] = [:],
    fonts: [UInt32: Font] = [:]
  ) {
    self.store = store
    self.bridge = bridge
    self.theme = EweTheme(colors: colors, fonts: fonts)
  }
  public init(store: NodeStore, bridge: EventBridge, theme: EweTheme) {
    self.store = store
    self.bridge = bridge
    self.theme = theme
  }
  public var body: some View {
    root.frame(maxWidth: .infinity, maxHeight: .infinity).eweLayerHost()
  }
  private var root: AnyView {
    #if os(iOS)
      if let root = store.record(1), let navigation = rootNavigation(root) { return navigation }
    #endif
    return node(1)
  }
  func records(_ node: NodeRecord) -> [NodeRecord] {
    node.children.compactMap(store.record).filter { $0.props[.hidden] != .bool(true) }
  }
  private func children(_ n: NodeRecord) -> some View {
    ForEach(n.children.compactMap(store.record), id: \.identity) { child in
      RecordView(screen: self, n: child)
    }
  }
  private func node(_ id: UInt32) -> AnyView {
    guard let n = store.record(id) else { return AnyView(EmptyView()) }
    return AnyView(RecordView(screen: self, n: n))
  }
  @MainActor struct RecordView: View {
    let screen: EweScreen
    let n: NodeRecord
    @State var input = ""
    @Environment(\.eweSheetDraft) private var sheetDraft
    var body: some View {
      #if os(iOS)
        if n.props[.hidden] != .bool(true), let presentation = screen.presentation(n) {
          return AnyView(presentation.modifier(EweNodeStyle(theme: screen.theme, node: n)))
        }
      #endif
      if n.kind == .overlay, !n.behaviours.contains("DIALOG") {
        return AnyView(EweOverlay(identity: n.identity) { ordinary })
      }
      return AnyView(ordinary)
    }
    private var ordinary: some View {
      let content: AnyView
      switch n.kind {
      case .stack:
        if n.props[.axis] == .axis("Horizontal") {
          content = AnyView(
            HStack(alignment: n.verticalAlignment, spacing: n.points(.spacing)) {
              screen.children(n)
            })
        } else {
          content = AnyView(
            EweColumn(nodes: screen.records(n), alignment: n.horizontalAlignment, spacing: n.points(.spacing)) { child in
              RecordView(screen: screen, n: child)
            })
        }
      case .overlay: content = AnyView(ZStack(alignment: .bottom) { screen.children(n) })
      case .modal:
        content = AnyView(EweModal(screen: screen, node: n) { screen.children(n) })
      case .bottomSheet:
        content = AnyView(EweBottomSheet {
          EweSheetChildren(nodes: screen.records(n)) { RecordView(screen: screen, n: $0) }
        })
      case .scroll: content = AnyView(ScrollView { screen.children(n) })
      case .list:
        content = AnyView(
          ScrollView {
            LazyVStack(alignment: n.horizontalAlignment, spacing: n.points(.spacing)) {
              screen.children(n)
            }
          })
      case .text: content = AnyView(Text(n.text ?? ""))
      case .sheetTitle: content = AnyView(EweSheetTitle(node: n))
      case .navigationTitle: content = AnyView(EweNavigationTitle(node: n))
      case .listFooter: content = AnyView(EweListFooter(node: n))
      case .image:
        if case .text(let source) = n.props[.source] {
          content = AnyView(AsyncImage(url: URL(string: source)))
        } else {
          content = AnyView(EmptyView())
        }
      case .button:
        content = AnyView(Button(n.text ?? "") { screen.sendButton(n, draft: sheetDraft) })
      case .submitButton:
        content = AnyView(EweSubmitButton(node: n) { screen.sendButton(n, draft: sheetDraft) })
      case .cancelButton:
        content = AnyView(EweCancelButton(node: n) { screen.sendButton(n, draft: sheetDraft) })
      case .destructiveButton:
        content = AnyView(EweDestructiveButton(node: n) { screen.sendButton(n, draft: sheetDraft) })
      case .toolbarButton:
        content = AnyView(EweToolbarButton(node: n) { screen.sendButton(n, draft: sheetDraft) })
      case .checkmarkButton:
        content = AnyView(EweCheckmarkButton(node: n, checked: n.props[.checked] == .bool(true)) {
          screen.sendButton(n, draft: sheetDraft)
        })
      case .textInput:
        let text = Binding<String>(
          get: { sheetDraft?.value(for: n) ?? input },
          set: { value in
            if let sheetDraft { sheetDraft.set(value, for: n) } else { input = value }
            screen.bridge.send(.change, node: n.id, value: value)
          })
        let field = TextField(n.props[.placeholder]?.description ?? "", text: text)
          .onSubmit { screen.bridge.send(.submit, node: n.id, value: text.wrappedValue) }
          .onAppear { input = n.props[.value]?.description ?? "" }
          .onChange(of: n.props[.value]) { _, _ in sheetDraft?.set(nil, for: n) }
          .onDisappear {
            if screen.store.record(n.id) !== n { sheetDraft?.set(nil, for: n) }
          }
        content = sheetDraft == nil ? AnyView(field) : AnyView(field.textFieldStyle(.roundedBorder))
      case .toggle:
        content = AnyView(
          Toggle(
            n.text ?? "",
            isOn: Binding(
              get: { n.props[.checked] == .bool(true) },
              set: { screen.bridge.send(.change, node: n.id, value: String($0), checked: $0) })))
      case .spacer: content = AnyView(Spacer())
      default:
        content = AnyView(
          Color.clear.frame(width: 0, height: 0).accessibilityElement(children: .ignore)
            .accessibilityLabel("Unsupported content"))
      }
      let identified: AnyView
      switch n.kind {
      case .list, .stack, .overlay, .scroll, .bottomSheet:
        identified = AnyView(content.accessibilityElement(children: .contain))
      default: identified = content
      }
      let styled = identified.modifier(NodeSize(node: n))
        .modifier(EweNodeStyle(theme: screen.theme, node: n))
        .disabled(n.props[.enabled] == .bool(false))
        .accessibilityIdentifier("ewe-node-\(n.id)")
      let accessible: AnyView
      if case .text(let label) = n.props[.a11yLabel] {
        accessible = AnyView(styled.accessibilityLabel(label))
      } else {
        accessible = AnyView(styled)
      }
      return Group {
        if n.props[.hidden] != .bool(true) { screen.store.behaviours.apply(accessible, to: n) }
      }
    }
  }

}
@MainActor private struct NodeSize: ViewModifier {
  let node: NodeRecord
  func body(content: Content) -> some View {
    var view = AnyView(content.frame(width: node.points(.width), height: node.points(.height)))
    if case .length(let value, "%") = node.props[.width] {
      view = AnyView(
        view.containerRelativeFrame(.horizontal) { size, _ in max(0, size * CGFloat(value) / 100) })
    }
    if case .length(let value, "%") = node.props[.height] {
      view = AnyView(
        view.containerRelativeFrame(.vertical) { size, _ in max(0, size * CGFloat(value) / 100) })
    }
    return view
  }
}
extension NodeRecord {
  fileprivate var horizontalAlignment: HorizontalAlignment {
    switch props[.align] {
    case .alignment("Start"): .leading
    case .alignment("End"): .trailing
    default: .center
    }
  }
  fileprivate var verticalAlignment: VerticalAlignment {
    switch props[.align] {
    case .alignment("Start"): .top
    case .alignment("End"): .bottom
    default: .center
    }
  }
  var identity: ObjectIdentifier { ObjectIdentifier(self) }
  fileprivate func points(_ prop: PropId) -> CGFloat? {
    if case .length(let value, "pt") = props[prop] { return CGFloat(value) }
    return nil
  }
}
#if canImport(UIKit)
  import UIKit
  @MainActor public final class SystemCommandHost: CommandHost {
    private let presenter: () -> UIViewController?
    public init(presenter: @escaping () -> UIViewController?) { self.presenter = presenter }
    public func run(_ command: ewe_ui_NativeCommandId, args: [String: Value]) -> Bool {
      switch command {
      case .haptic:
        UIImpactFeedbackGenerator().impactOccurred()
        return true
      case .openUrl:
        guard case .text(let string) = args["url"], let url = URL(string: string) else {
          return false
        }
        UIApplication.shared.open(url)
        return true
      case .share:
        guard case .text(let text) = args["text"], let parent = presenter() else { return false }
        let controller = UIActivityViewController(activityItems: [text], applicationActivities: nil)
        controller.popoverPresentationController?.sourceView = parent.view
        parent.present(controller, animated: true)
        return true
      default: return false
      }
    }
  }
#endif
