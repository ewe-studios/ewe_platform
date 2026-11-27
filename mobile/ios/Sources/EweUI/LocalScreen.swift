import Foundation
import SwiftUI

/// Loads a frame and an action core supplied by the application.
@MainActor public struct LocalScreen: View {
  public typealias Loader = @MainActor (NodeStore) async throws -> (any ActionCore, Data)

  #if os(iOS)
    @State private var presenter = PresentationAnchor()
  #endif
  private let load: Loader
  @State private var connection: EventBridge?
  private let theme: EweTheme
  private let onViewReady: @MainActor (EventBridge) -> Void
  @State private var issue: String?

  public init(
    theme: EweTheme = EweTheme(),
    onViewReady: @escaping @MainActor (EventBridge) -> Void = { _ in },
    load: @escaping Loader
  ) {
    self.theme = theme
    self.onViewReady = onViewReady
    self.load = load
  }

  public var body: some View {
    Group {
      if let connection {
        EweScreen(store: connection.store, bridge: connection, theme: theme).safeAreaInset(edge: .bottom) {
          if let issue { Text(issue) }
        }
      } else if let issue {
        Text(issue)
      } else {
        ProgressView()
      }
    }
    #if os(iOS)
      .background(HostAnchor(anchor: presenter).frame(width: 0, height: 0))
    #endif
    .task {
      guard connection == nil else { return }
      let store = NodeStore()
      #if os(iOS)
        store.commands = presenter.commandHost()
      #endif
      store.onError = { code, message in issue = code + " " + message }
      do {
        let (core, first) = try await load(store)
        try Task.checkCancellation()
        let bridge = EventBridge(store: store, core: core)
        guard store.receive(first) else { throw CocoaError(.fileReadCorruptFile) }
        connection = bridge
        onViewReady(bridge)
      } catch {
        if !Task.isCancelled { issue = error.localizedDescription }
      }
    }
  }
}
