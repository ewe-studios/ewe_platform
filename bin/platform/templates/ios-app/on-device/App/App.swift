import EweUI
import SwiftUI

@main struct {{ PROJECT_NAME }}App: App {
    var body: some Scene {
        WindowGroup {
            LocalScreen { store in
                let core = try createCounterCore(platform: .ios)
                let first = try await initialCounterFrame(core: core)
                return (NativeActionCore(core, store: store), first)
            }
        }
    }
}
