import EweTodoCore
import EweUI
import SwiftUI

@main struct TodoServerApp: App {
  @StateObject private var game = MathalleyRoute()
  private let theme = Result {
    if ProcessInfo.processInfo.environment["EWE_TODO_THEME"] == "branded" {
      return try EweTheme(manifest: Data(todoThemeManifest().utf8))
    }
    return EweTheme()
  }

  var body: some Scene {
    WindowGroup {
      Group { switch theme {
      case .failure(let error): Text(error.localizedDescription)
      case .success(let theme):
        let address = ProcessInfo.processInfo.environment["EWE_TODO_URL"] ?? "http://127.0.0.1:8080"
        if let url = URL(string: address) {
          if ProcessInfo.processInfo.environment["EWE_TODO_RPC"] == "1" {
            ServerScreen(base: url, initial: "initial", rpcBase: url, theme: theme) { transport, store in
              try game.attach(createTodoCore(platform: .ios, transport: transport), store: store)
            }
          } else if ProcessInfo.processInfo.environment["EWE_TODO_SOCKET"] == "1" {
            ServerScreen(base: url, initial: "initial", socket: url.appendingPathComponent("socket"), theme: theme) { transport, store in
              try game.attach(createTodoCore(platform: .ios, transport: transport), store: store)
            }
          } else {
          ServerScreen(base: url, initial: "initial", theme: theme) { transport, store in
            let core = try createTodoCore(platform: .ios, transport: transport)
            return try game.attach(core, store: store)
          }
          }
        }
      } }
      .fullScreenCover(isPresented: $game.visible, onDismiss: game.returned) {
        if let core = game.core { MathalleyScreen(core: core) }
      }
      .alert("Mathalley", isPresented: Binding(get: { game.error != nil }, set: { if !$0 { game.error = nil } })) {
        Button("OK") { game.error = nil }
      } message: { Text(game.error ?? "") }
    }
  }
}
