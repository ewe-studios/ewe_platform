import EweTodoCore
import EweUI
import SwiftUI

@main struct TodoServerApp: App {
  private let theme = Result {
    if ProcessInfo.processInfo.environment["EWE_TODO_THEME"] == "branded" {
      return try EweTheme(manifest: Data(todoThemeManifest().utf8))
    }
    return EweTheme()
  }

  var body: some Scene {
    WindowGroup {
      switch theme {
      case .failure(let error): Text(error.localizedDescription)
      case .success(let theme):
        let address = ProcessInfo.processInfo.environment["EWE_TODO_URL"] ?? "http://127.0.0.1:8080"
        if let url = URL(string: address) {
          if ProcessInfo.processInfo.environment["EWE_TODO_RPC"] == "1" {
            ServerScreen(base: url, initial: "native-initial", rpcBase: url, theme: theme) { transport, store in
              NativeActionCore(try createTodoCore(platform: .ios, transport: transport), store: store)
            }
          } else {
          ServerScreen(base: url, initial: "native-initial", theme: theme) { transport, store in
            let core = try createTodoCore(platform: .ios, transport: transport)
            return NativeActionCore(core, store: store)
          }
          }
        }
      }
    }
  }
}
