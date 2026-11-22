import EweTodoCore
import EweUI
import SwiftUI

@main struct TodoOnDeviceApp: App {
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
        LocalScreen(theme: theme) { store in
          let core = try createLocalTodoCore(platform: .ios)
          let first = try await initialTodoFrame(core: core)
          return (try game.attach(core, store: store), first)
        }
        .preferredColorScheme(ProcessInfo.processInfo.environment["EWE_TODO_APPEARANCE"].flatMap {
          $0 == "dark" ? .dark : $0 == "light" ? .light : nil
        })
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
