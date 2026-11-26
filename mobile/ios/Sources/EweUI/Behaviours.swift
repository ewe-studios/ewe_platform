import SwiftUI

/// The host can select the transition time and swipe distance for its theme.
public struct MotionSettings: Sendable {
  public var duration: TimeInterval
  public var dismissDistance: CGFloat
  public init(duration: TimeInterval = 0.2, dismissDistance: CGFloat = 60) {
    self.duration = duration
    self.dismissDistance = dismissDistance
  }
}

@MainActor struct ScrollLockBinding: ViewModifier {
  func body(content: Content) -> some View { content.scrollDisabled(true) }
}

@MainActor struct DialogBinding: View {
  let content: AnyView
  let allowsDismiss: Bool
  @State private var presented = false
  var body: some View {
    Color.clear.frame(width: 0, height: 0).onAppear { presented = true }
      .sheet(isPresented: $presented) {
        content.frame(maxWidth: .infinity, maxHeight: .infinity)
          .environment(\.eweDismiss, allowsDismiss ? { presented = false } : nil)
          .padding()
          .interactiveDismissDisabled(!allowsDismiss)
          #if os(iOS)
            .presentationDetents([.medium, .large])
            .presentationDragIndicator(allowsDismiss ? .visible : .hidden)
            .background(OutsideDismiss(enabled: allowsDismiss, close: { presented = false }))
          #endif
      }
  }
}

private struct DismissAction: EnvironmentKey {
  static let defaultValue: (() -> Void)? = nil
}
extension EnvironmentValues {
  fileprivate var eweDismiss: (() -> Void)? {
    get { self[DismissAction.self] }
    set { self[DismissAction.self] = newValue }
  }
}

@MainActor struct DismissBinding: ViewModifier {
  let motion: MotionSettings
  @Environment(\.eweDismiss) var close
  @State private var hidden = false
  func body(content: Content) -> some View {
    if !hidden {
      content
        .onKeyPress(.escape) {
          dismiss()
          return .handled
        }
        .accessibilityAction(.escape, dismiss)
        .simultaneousGesture(
          DragGesture().onEnded { value in
            if value.translation.height > motion.dismissDistance
              && value.translation.height > abs(value.translation.width)
            {
              dismiss()
            }
          })
    }
  }
  private func dismiss() { if let close { close() } else { hidden = true } }
}

@MainActor struct TransitionBinding: ViewModifier {
  let node: NodeRecord
  let motion: MotionSettings
  @Environment(\.accessibilityReduceMotion) var reduceMotion
  func body(content: Content) -> some View {
    content.transition(reduceMotion ? .identity : .opacity)
      .animation(reduceMotion ? nil : .easeInOut(duration: motion.duration), value: node.children)
  }
}

#if os(iOS)
  import UIKit
  /// The presentation container also receives taps outside a partly expanded sheet.
  private struct OutsideDismiss: UIViewRepresentable {
    var enabled: Bool
    var close: () -> Void
    func makeUIView(context: Context) -> Hook { Hook() }
    func updateUIView(_ view: Hook, context: Context) { view.close = enabled ? close : nil }
    static func dismantleUIView(_ view: Hook, coordinator: ()) { view.detach() }
    final class Hook: UIView, UIGestureRecognizerDelegate {
      var close: (() -> Void)?
      private var recognizer: UITapGestureRecognizer?
      override func didMoveToWindow() {
        super.didMoveToWindow()
        detach()
        guard let window else { return }
        let tap = UITapGestureRecognizer(target: self, action: #selector(outside(_:)))
        tap.cancelsTouchesInView = false
        tap.delegate = self
        window.addGestureRecognizer(tap)
        recognizer = tap
      }
      func detach() {
        if let recognizer { recognizer.view?.removeGestureRecognizer(recognizer) }
        recognizer = nil
      }
      func gestureRecognizer(
        _ gestureRecognizer: UIGestureRecognizer,
        shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
      ) -> Bool { true }
      @objc private func outside(_ tap: UITapGestureRecognizer) {
        guard let close, window != nil else { return }
        var responder: UIResponder? = self
        while let current = responder {
          if let controller = current as? UIViewController,
            let sheet = controller.presentationController?.presentedView
          {
            if !sheet.bounds.contains(tap.location(in: sheet)) { close() }
            return
          }
          responder = current.next
        }
      }
    }
  }
#endif
