import EweTodoCore
import EweUI
import SwiftUI

@main struct TodoOnDeviceApp: App {
  var body: some Scene {
    WindowGroup {
      LocalScreen { store in
        let core = try createLocalTodoCore(platform: .ios)
        let first = try await initialTodoFrame(core: core)
        return (NativeActionCore(core, store: store), first)
      }
    }
  }
}
