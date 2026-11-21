import SwiftUI

/// A trailing panel uses its measured height. The other children share the available space.
@MainActor struct EweColumn<Child: View>: View {
  let nodes: [NodeRecord]
  let alignment: HorizontalAlignment
  let spacing: CGFloat?
  @ViewBuilder let child: (NodeRecord) -> Child

  var body: some View {
    VStack(alignment: alignment, spacing: spacing) {
      if let panel = nodes.last, panel.kind == .bottomSheet {
        VStack(alignment: alignment, spacing: spacing) {
          ForEach(nodes.dropLast(), id: \.identity, content: child)
        }
        .frame(maxHeight: .infinity, alignment: .top)
        child(panel).layoutPriority(1)
      } else {
        ForEach(nodes, id: \.identity, content: child)
      }
    }
  }
}

/// A layer carries its children to the nearest screen or modal surface.
@MainActor struct EweOverlay<Content: View>: View {
  let identity: ObjectIdentifier
  @ViewBuilder let content: () -> Content

  var body: some View {
    Color.clear.frame(width: 0, height: 0)
      .accessibilityHidden(true)
      .preference(key: ScreenLayers.self, value: [Layer(id: identity, content: AnyView(content()))])
  }
}

private struct Layer: Identifiable {
  let id: ObjectIdentifier
  let content: AnyView
}

private enum ScreenLayers: PreferenceKey {
  static var defaultValue: [Layer] { [] }
  static func reduce(value: inout [Layer], nextValue: () -> [Layer]) {
    value.append(contentsOf: nextValue())
  }
}

extension View {
  /// Position layers above this surface without changing the measured size of their parent.
  func eweLayerHost() -> some View {
    overlayPreferenceValue(ScreenLayers.self, alignment: .bottom) { layers in
      ZStack(alignment: .bottom) {
        ForEach(layers) { $0.content }
      }
    }
    .transformPreference(ScreenLayers.self) { $0.removeAll() }
  }
}

/// A modal blocks the parent screen and presents its children on one surface.
@MainActor struct EweModal<Content: View>: View {
  let screen: EweScreen
  let node: NodeRecord
  @ViewBuilder let content: () -> Content
  @State private var presented = false

  var body: some View {
    let anchor = Color.clear.frame(width: 0, height: 0)
      .accessibilityHidden(true)
      .onAppear { presented = true }
    #if os(iOS)
      anchor.fullScreenCover(isPresented: $presented) {
        surface.presentationBackground(.clear)
      }
    #else
      anchor.sheet(isPresented: $presented) { surface }
    #endif
  }

  private var bottomAligned: Bool { screen.records(node).contains { $0.kind == .bottomSheet } }

  func cancelControl(in container: NodeRecord) -> NodeRecord? {
    for child in screen.records(container) {
      if child.kind == .cancelButton, child.actions[.tap] != nil,
        child.props[.enabled] != .bool(false) { return child }
      if child.kind != .modal, let cancel = cancelControl(in: child) { return cancel }
    }
    return nil
  }

  private func dismiss() {
    guard let control = cancelControl(in: node) else { return }
    screen.bridge.send(.tap, node: control.id)
  }

  private var surface: some View {
    ZStack(alignment: bottomAligned ? .bottom : .center) {
      Color.black.opacity(0.32)
        .ignoresSafeArea()
        .onTapGesture(perform: dismiss)
        .accessibilityHidden(true)
      VStack { content() }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .eweLayerHost()
    .interactiveDismissDisabled()
    .accessibilityAction(.escape, dismiss)
  }
}

@Observable @MainActor final class SheetDraft {
  private var values: [ObjectIdentifier: String] = [:]

  func set(_ value: String?, for field: NodeRecord) {
    values[field.identity] = value
  }

  func value(for field: NodeRecord) -> String {
    values[field.identity] ?? field.props[.value]?.description ?? ""
  }
}

private enum SheetDraftKey: EnvironmentKey {
  static let defaultValue: SheetDraft? = nil
}
extension EnvironmentValues {
  var eweSheetDraft: SheetDraft? {
    get { self[SheetDraftKey.self] }
    set { self[SheetDraftKey.self] = newValue }
  }
}

/// A bottom sheet measures its children and keeps their declared order.
@MainActor struct EweBottomSheet<Content: View>: View {
  @ViewBuilder let content: () -> Content
  @State private var draft = SheetDraft()

  var body: some View {
    ViewThatFits(in: .vertical) {
      panel.fixedSize(horizontal: false, vertical: true)
      ScrollView { panel }
    }
    .frame(maxWidth: .infinity)
    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 20))
    .environment(\.eweSheetDraft, draft)
  }

  private var panel: some View {
    VStack(alignment: .leading, spacing: 12) { content() }
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding()
  }
}

/// Keep the input and its actions in the same draft scope.
@MainActor struct EweSheetChildren<Child: View>: View {
  let nodes: [NodeRecord]
  @ViewBuilder let child: (NodeRecord) -> Child

  var body: some View {
    let pair = zip(nodes, nodes.dropFirst()).first { first, second in
      Set([first.kind, second.kind]) == [.cancelButton, .submitButton]
    }
    ForEach(nodes, id: \.identity) { node in
      if let (first, second) = pair, node === first {
        let cancelFirst = first.kind == .cancelButton
        let cancel = cancelFirst ? first : second
        let submit = cancelFirst ? second : first
        ViewThatFits(in: .horizontal) {
          HStack(spacing: 12) {
            child(cancel)
            child(submit)
          }.fixedSize()
          VStack(alignment: .trailing) {
            child(cancel)
            child(submit)
          }
        }
        .frame(maxWidth: .infinity, alignment: .trailing)
      } else if node !== pair?.1 {
        child(node)
      }
    }
  }
}

@MainActor extension EweScreen {
  func sendButton(_ button: NodeRecord, draft: SheetDraft?) {
    guard let draft, button.kind == .submitButton else {
      bridge.send(.tap, node: button.id, value: button.props[.value]?.description ?? "")
      return
    }
    let field = submitField(for: button)
    if button.actions[.tap] != nil {
      bridge.send(.tap, node: button.id, value: field.map(draft.value(for:)) ?? "")
    } else if let field {
      bridge.send(.submit, node: field.id, value: draft.value(for: field))
    }
  }

  func submitField(for button: NodeRecord) -> NodeRecord? {
    guard let parent = button.parent.flatMap(store.record) else { return nil }
    let inputs = records(parent).filter {
      $0.kind == .textInput && $0.actions[.submit] != nil && $0.props[.enabled] != .bool(false)
    }
    return inputs.count == 1 ? inputs.first : nil
  }

}
