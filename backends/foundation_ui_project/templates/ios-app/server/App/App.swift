import EweUI
import SwiftUI

@main struct {{ PROJECT_NAME }}App: App {
    private static let defaultServer = "http://127.0.0.1:8080"
    @State private var address = ProcessInfo.processInfo.environment["EWE_SERVER"] ?? defaultServer

    var body: some Scene {
        WindowGroup {
            if let endpoint = URL(string: address), let host = endpoint.host, !host.isEmpty,
               let scheme = endpoint.scheme?.lowercased(), ["http", "https"].contains(scheme) {
                ServerScreen(base: endpoint, initial: "open-count", actions: ["refresh_count"])
            } else {
{% if COMPOSED %}                EweStartupView("Connecting", issue: "Invalid server address. Use an http or https address.",
                               retryTitle: "Use default server") { address = Self.defaultServer }
{% else %}                Text("Invalid server address")
{% endif %}            }
        }
    }
}
