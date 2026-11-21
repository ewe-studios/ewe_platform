import EweUI
import SwiftUI

@main struct StarterApp: App {
  var body: some Scene {
    WindowGroup {
      ServerScreen(
        base: URL(string: "http://127.0.0.1:8080")!,
        initial: "native-starter", actions: ["refresh_count"])
    }
  }
}
