#if os(iOS) || os(macOS)
  import EweCore
  import Foundation
  import SwiftUI

  /// Opens the action connection and applies the initial native view.
  @MainActor public struct ServerScreen: View {
    public typealias CoreFactory =
      @MainActor (URLActionTransport, NodeStore) throws -> NativeActionCore
    #if os(iOS)
      @State private var presenter = PresentationAnchor()
    #endif
    let base: URL
    let initial: String
    let create: CoreFactory
    let session: URLSession
    @State private var screen: EweScreen?
    @State private var issue: String?
    @State private var attempt = 0

    public init(
      base: URL, initial: String, session: URLSession = .shared,
      create: @escaping CoreFactory
    ) {
      self.base = base
      self.initial = initial
      self.session = session
      self.create = create
    }

    public init(base: URL, initial: String, actions: [String], session: URLSession = .shared) {
      self.init(base: base, initial: initial, session: session) { transport, store in
        let core = try NativeCore.remote(platform: .ios, names: actions, transport: transport)
        return NativeActionCore(core, store: store)
      }
    }

    public var body: some View {
      Group {
        if let screen {
          screen.safeAreaInset(edge: .bottom) {
            if let issue { connectionIssue(issue) }
          }
        } else if let issue {
          connectionIssue(issue)
        } else {
          ProgressView("Connecting")
        }
      }
      #if os(iOS)
        .background(HostAnchor(anchor: presenter).frame(width: 0, height: 0))
      #endif
      .task(id: attempt) { await connect() }.onDisappear { screen?.store.liveHost?.close() }
    }

    private func connectionIssue(_ message: String) -> some View {
      VStack {
        Text(message)
        Button("Reconnect") { attempt += 1 }
      }
    }

    private func connect() async {
      screen?.store.liveHost?.close()
      issue = nil
      let live = LiveMountHost(factory: URLSessionChannelFactory(session: session, baseURL: base))
      let store = NodeStore(live: live)
      #if os(iOS)
        store.commands = presenter.commandHost()
      #endif
      do {
        let endpoint = base.appending(path: "action")
        let transport = URLActionTransport(maxFrameBytes: store.maxFrameBytes, session: session) {
          URLRequest(url: endpoint)
        }
        let core = try create(transport, store)
        try await core.negotiate(transport)
        let bridge = EventBridge(store: store, core: core)
        let (bytes, response) = try await session.bytes(from: base.appending(path: initial))
        guard let status = (response as? HTTPURLResponse)?.statusCode, (200..<300).contains(status)
        else {
          throw URLError(.badServerResponse)
        }
        var first = Data()
        for try await byte in bytes {
          guard first.count < store.maxFrameBytes else { throw URLError(.dataLengthExceedsMaximum) }
          first.append(byte)
        }
        guard store.receive(first) else { throw URLError(.cannotParseResponse) }
        store.onError = { code, message in self.issue = code + " " + message }
        screen = EweScreen(store: store, bridge: bridge)
      } catch { if !Task.isCancelled { issue = error.localizedDescription } }
    }
  }
#endif

#if os(iOS)
  import UIKit
  @MainActor final class PresentationAnchor {
    weak var controller: UIViewController?

    func commandHost() -> SystemCommandHost {
      SystemCommandHost { [weak self] in
        var presenter = self?.controller
        while let presented = presenter?.presentedViewController { presenter = presented }
        return presenter
      }
    }
  }

  @MainActor struct HostAnchor: UIViewControllerRepresentable {
    let anchor: PresentationAnchor
    func makeUIViewController(context: Context) -> UIViewController {
      let controller = UIViewController()
      anchor.controller = controller
      return controller
    }
    func updateUIViewController(_ controller: UIViewController, context: Context) {
      anchor.controller = controller.parent ?? controller
    }
  }
#endif
