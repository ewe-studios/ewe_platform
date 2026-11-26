#if os(iOS)
  import SwiftUI
  import UIKit

  /// Display the system search field at the position specified by the application.
  @MainActor struct EweSearchField: UIViewRepresentable {
    let node: NodeRecord
    let bridge: EventBridge

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> UISearchBar {
      let bar = UISearchBar()
      bar.searchBarStyle = .minimal
      bar.autocapitalizationType = .none
      bar.returnKeyType = .search
      bar.searchTextField.font = .preferredFont(forTextStyle: .body)
      bar.searchTextField.adjustsFontForContentSizeCategory = true
      bar.delegate = context.coordinator
      return bar
    }

    func updateUIView(_ bar: UISearchBar, context: Context) {
      let coordinator = context.coordinator
      coordinator.field = self
      bar.placeholder = node.props[.placeholder]?.description
      bar.searchTextField.accessibilityLabel = node.props[.a11yLabel]?.description
      bar.searchTextField.isEnabled = node.isEnabled
      if coordinator.modelValue != node.value {
        coordinator.modelValue = node.value
        if bar.text != node.value { bar.text = node.value }
      }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: UISearchBar, context: Context) -> CGSize? {
      guard let width = proposal.width else { return nil }
      let size = uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude))
      return CGSize(width: width, height: size.height)
    }

    @MainActor final class Coordinator: NSObject, UISearchBarDelegate {
      var field: EweSearchField
      var modelValue: String?
      init(_ field: EweSearchField) { self.field = field }

      func searchBar(_ searchBar: UISearchBar, textDidChange text: String) {
        field.bridge.send(.change, node: field.node.id, value: text)
      }

      func searchBarSearchButtonClicked(_ searchBar: UISearchBar) {
        searchBar.resignFirstResponder()
        if field.node.actions[.submit] != nil {
          field.bridge.send(.submit, node: field.node.id, value: searchBar.text ?? "")
        }
      }
    }
  }
#endif
