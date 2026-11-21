import EweWire
import SwiftUI

@MainActor public struct EweScreen: View {
  public let store: NodeStore
  public let bridge: EventBridge
  private let colors: [UInt32: Color]
  private let fonts: [UInt32: Font]
  public init(
    store: NodeStore, bridge: EventBridge, colors: [UInt32: Color] = [:],
    fonts: [UInt32: Font] = [:]
  ) {
    self.store = store
    self.bridge = bridge
    self.colors = colors
    self.fonts = fonts
  }
  public var body: some View {
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
    var body: some View {
      #if os(iOS)
        if n.props[.hidden] != .bool(true), let presentation = screen.presentation(n) {
          return presentation
        }
      #endif
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
            VStack(alignment: n.horizontalAlignment, spacing: n.points(.spacing)) {
              screen.children(n)
            })
        }
      case .overlay: content = AnyView(ZStack { screen.children(n) })
      case .scroll: content = AnyView(ScrollView { screen.children(n) })
      case .list:
        content = AnyView(
          ScrollView {
            LazyVStack(alignment: n.horizontalAlignment, spacing: n.points(.spacing)) {
              screen.children(n)
            }
          })
      case .text: content = AnyView(Text(n.text ?? ""))
      case .image:
        if case .text(let source) = n.props[.source] {
          content = AnyView(AsyncImage(url: URL(string: source)))
        } else {
          content = AnyView(EmptyView())
        }
      case .button:
        content = AnyView(
          Button(n.text ?? "") {
            screen.bridge.send(.tap, node: n.id, value: n.props[.value]?.description ?? "")
          })
      case .textInput:
        content = AnyView(
          TextField(n.props[.placeholder]?.description ?? "", text: $input)
            .onChange(of: input) { _, value in screen.bridge.send(.change, node: n.id, value: value)
            }
            .onSubmit { screen.bridge.send(.submit, node: n.id, value: input) }
            .onAppear { input = n.props[.value]?.description ?? "" })
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
      case .list, .stack, .overlay, .scroll:
        identified = AnyView(content.accessibilityElement(children: .contain))
      default: identified = content
      }
      let styled = identified.modifier(NodeSize(node: n))
        .padding(n.points(.padding) ?? 0)
        .foregroundStyle(screen.color(n.props[.color]) ?? .primary)
        .background(screen.color(n.props[.background]) ?? .clear)
        .font(screen.font(n.props[.font]))
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
  private func color(_ prop: Value?) -> Color? {
    guard case .color(let id) = prop else { return nil }
    return colors[id]
  }
  private func font(_ prop: Value?) -> Font? {
    guard case .font(let id) = prop else { return nil }
    return fonts[id] ?? .body
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
