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

  private func actionTransport(_ base: URL) -> ActionTransport {
    let environment = ProcessInfo.processInfo.environment
    if environment["EWE_TODO_RPC"] == "1" { return .connect(base) }
    if environment["EWE_TODO_SOCKET"] == "1" { return .webSocket(base.appendingPathComponent("socket")) }
    return .http
  }

  var body: some Scene {
    WindowGroup {
      switch theme {
      case .failure(let error): Text(error.localizedDescription)
      case .success(let theme):
        let address = ProcessInfo.processInfo.environment["EWE_TODO_URL"]
          ?? Bundle.main.object(forInfoDictionaryKey: "EweTodoServerURL") as? String
        if let address, let url = URL(string: address),
          let scheme = url.scheme?.lowercased(), ["http", "https"].contains(scheme), url.host?.isEmpty == false
        {
          let key = SHA256.hash(data: Data(url.absoluteString.utf8)).map { String(format: "%02x", $0) }.joined()
          let file = URL.applicationSupportDirectory.appending(path: "TodoPending/\(key).json").path
          ServerScreen(base: url, initial: "initial", transport: actionTransport(url), theme: theme) { transport, store in
            let core = try createPersistentTodoCore(platform: .ios, transport: transport, storageFile: file)
            core.installResources(source: BundleResourceSource())
            return NativeActionCore(core, store: store, viewDataAction: "host:apply_todo_delivery")
          }
        } else {
          ContentUnavailableView("Invalid Todo server address", systemImage: "exclamationmark.triangle",
            description: Text("Set EWE_TODO_URL to the full server address."))
        }
      }
    }
  }
}
