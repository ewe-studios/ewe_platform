import EweTodoCore
import EweUI
import SwiftUI

@main struct TodoServerApp: App {
  var body: some Scene {
    WindowGroup {
      let address = ProcessInfo.processInfo.environment["EWE_TODO_URL"] ?? "http://127.0.0.1:8080"
      if let url = URL(string: address) {
        ServerScreen(base: url, initial: "native-initial") { transport, store in
          let core = try createTodoCore(platform: .ios, transport: transport)
          return NativeActionCore(core, store: store)
        }
      }
    }
  }
}
