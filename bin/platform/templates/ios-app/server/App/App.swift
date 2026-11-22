import EweUI
import SwiftUI

@main struct {{ PROJECT_NAME }}App: App {
    var body: some Scene {
        WindowGroup {
            let address = ProcessInfo.processInfo.environment["EWE_SERVER"] ?? "http://127.0.0.1:8080"
            if let endpoint = URL(string: address), endpoint.host != nil {
                ServerScreen(base: endpoint, initial: "open-count", actions: ["refresh_count"])
            } else {
                Text("Invalid server address")
            }
        }
    }
}
