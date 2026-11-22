import CryptoKit
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
          let key = SHA256.hash(data: Data(url.absoluteString.utf8)).map { String(format: "%02x", $0) }.joined()
          let file = URL.applicationSupportDirectory.appending(path: "TodoPending/\(key).json").path
          if ProcessInfo.processInfo.environment["EWE_TODO_RPC"] == "1" {
            ServerScreen(base: url, initial: "initial", rpcBase: url, theme: theme) { transport, store in
              NativeActionCore(try createPersistentTodoCore(platform: .ios, transport: transport, storageFile: file), store: store, viewDataAction: "host:apply_todo_delivery")
            }
          } else if ProcessInfo.processInfo.environment["EWE_TODO_SOCKET"] == "1" {
            ServerScreen(base: url, initial: "initial", socket: url.appendingPathComponent("socket"), theme: theme) { transport, store in
              NativeActionCore(try createPersistentTodoCore(platform: .ios, transport: transport, storageFile: file), store: store, viewDataAction: "host:apply_todo_delivery")
            }
          } else {
          ServerScreen(base: url, initial: "initial", theme: theme) { transport, store in
            let core = try createPersistentTodoCore(platform: .ios, transport: transport, storageFile: file)
            return NativeActionCore(core, store: store, viewDataAction: "host:apply_todo_delivery")
          }
          }
        }
      }
    }
  }
}
