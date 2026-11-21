import SwiftUI

@MainActor struct EweNavigationTitle: View {
  let node: NodeRecord
  var body: some View { Text(node.text ?? "").font(.title).accessibilityAddTraits(.isHeader) }
}

@MainActor struct EweSheetTitle: View {
  let node: NodeRecord
  var body: some View { Text(node.text ?? "").font(.headline).accessibilityAddTraits(.isHeader) }
}

@MainActor struct EweListFooter: View {
  let node: NodeRecord
  var body: some View { Text(node.text ?? "").font(.footnote).foregroundStyle(.secondary) }
}

@MainActor struct EweToolbarButton: View {
  let node: NodeRecord
  let action: () -> Void
  var body: some View {
    Button(node.text ?? "", action: action)
      .disabled(node.props[.enabled] == .bool(false))
      .accessibilityLabel(node.props[.a11yLabel]?.description ?? node.text ?? "")
  }
}

@MainActor struct EweSubmitButton: View {
  let node: NodeRecord
  let action: () -> Void
  var body: some View { Button(node.text ?? "", action: action).buttonStyle(.borderedProminent) }
}

@MainActor struct EweCancelButton: View {
  let node: NodeRecord
  let action: () -> Void
  var body: some View { Button(node.text ?? "", role: .cancel, action: action) }
}

@MainActor struct EweDestructiveButton: View {
  let node: NodeRecord
  let action: () -> Void
  var body: some View { Button(node.text ?? "", role: .destructive, action: action) }
}

@MainActor struct EweCheckmarkButton: View {
  let node: NodeRecord
  let checked: Bool
  let action: () -> Void
  var body: some View {
    Button(action: action) {
      Image(systemName: checked ? "checkmark.circle.fill" : "circle")
        .font(.title3)
        .foregroundStyle(checked ? Color.secondary : Color.accentColor)
        .frame(minWidth: 44, minHeight: 44)
        .contentShape(Rectangle())
    }
    .buttonStyle(.borderless)
    .disabled(node.props[.enabled] == .bool(false))
    .accessibilityLabel(node.props[.a11yLabel]?.description ?? node.text ?? "")
  }
}
