import EweTodoCore
import EweUI
import SwiftUI

@main struct TodoOnDeviceApp: App {
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
        LocalScreen(theme: theme) { store in
          let core = try createLocalTodoCore(platform: .ios)
          core.installResources(source: BundleResourceSource())
          let first = try await initialTodoFrame(core: core)
          return (NativeActionCore(core, store: store), first)
        }
        .preferredColorScheme(ProcessInfo.processInfo.environment["EWE_TODO_APPEARANCE"].flatMap {
          $0 == "dark" ? .dark : $0 == "light" ? .light : nil
        })
      }
    }
  }
}
