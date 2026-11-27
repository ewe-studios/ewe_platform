#if os(iOS) || os(macOS)
  import EweCore
  import Foundation
  import SwiftUI

  /// Selects the sender for server actions.
  public enum ActionTransport {
    case http
    case connect(URL)
    case webSocket(URL)
  }

  /// Opens the action connection and applies the initial native view.
  @MainActor public struct ServerScreen: View {
    public typealias CoreFactory =
      @MainActor (HostTransport, NodeStore) throws -> NativeActionCore
    #if os(iOS)
      @State private var presenter = PresentationAnchor()
    #endif
    let base: URL
    let initial: String
    let prepare: @MainActor (URLActionTransport, NodeStore) async throws -> NativeActionCore
    private var managed: ConnectionManager?
    let session: URLSession
    @State private var connection: EventBridge?
    private let theme: EweTheme
    private let onViewReady: @MainActor (EventBridge) -> Void
    @State private var issue: String?
    @State private var attempt = 0

    public init(
      base: URL, initial: String, transport: ActionTransport = .http,
      connections: ConnectionManager? = nil, session: URLSession = .shared, theme: EweTheme = EweTheme(),
      onViewReady: @escaping @MainActor (EventBridge) -> Void = { _ in },
      create: @escaping CoreFactory
    ) {
      self.base = base
      self.initial = initial
      self.session = session
      self.theme = theme
      self.onViewReady = onViewReady
      self.managed = connections
      self.prepare = { viewTransport, store in
        let sender: HostTransport
        var socket: SocketActionConnection?
        switch transport {
        case .http:
          sender = viewTransport
        case .connect(let endpoint):
          sender = try ConnectActionSender(base: endpoint, client: URLSessionRpcClient(session: session),
            maxFrameBytes: UInt32(store.maxFrameBytes))
        case .webSocket(let endpoint):
          guard let live = store.liveHost, let manager = live.connections else { throw ConnectionError.closed }
          let connection = try SocketActionConnection(manager: manager, endpoint: endpoint,
            onReply: { [weak store] bytes in _ = store?.receive(bytes) },
            onError: { [weak store] cause in store?.error("Transport", cause) })
          live.onClose { connection.close() }
          socket = connection
          sender = connection
        }
        do {
          let core = try create(sender, store)
          let hello = socket.map { core.raw.helloFor(requestTag: $0.requestTag) } ?? core.raw.hello()
          viewTransport.configure(hello: hello)
          guard let caps = try await sender.send(request: hello) else {
            throw CoreError.Transport(detail: "Missing server capabilities")
          }
          try core.applyCapsFrame(caps)
          return core
        } catch { socket?.close(); throw error }
      }
    }

    public init(base: URL, initial: String, actions: [String], session: URLSession = .shared, theme: EweTheme = EweTheme()) {
      self.init(base: base, initial: initial, session: session, theme: theme) { transport, store in
        let core = try NativeCore.remote(platform: .ios, names: actions, transport: transport)
        core.installResources(source: BundleResourceSource())
        return NativeActionCore(core, store: store)
      }
    }

    public var body: some View {
      Group {
        if let connection {
          EweScreen(store: connection.store, bridge: connection, theme: theme).safeAreaInset(edge: .bottom) {
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
      .task(id: attempt) { await connect() }.onDisappear { connection?.store.recovery?.close(); connection?.store.liveHost?.close() }
    }

    private func connectionIssue(_ message: String) -> some View {
      VStack {
        Text(message)
        Button("Reconnect") {
          if let recovery = connection?.store.recovery {
            Task {
              do { try await recovery.retry(); issue = nil }
              catch { issue = error.localizedDescription }
            }
          } else { attempt += 1 }
        }
      }
    }

    @MainActor private func readInitial(_ request: URLRequest, limit: Int) async throws -> Data {
        let (bytes, response) = try await session.bytes(for: request)
        guard let status = (response as? HTTPURLResponse)?.statusCode, (200..<300).contains(status)
        else {
          throw URLError(.badServerResponse)
        }
        var first = Data()
        for try await byte in bytes {
          guard first.count < limit else { throw URLError(.dataLengthExceedsMaximum) }
          first.append(byte)
        }
        return first
    }

    private func connect() async {
      connection?.store.recovery?.close()
      connection?.store.liveHost?.close()
      issue = nil
      let live = LiveMountHost(factory: URLSessionChannelFactory(session: session, baseURL: base), connections: managed)
      let store = NodeStore(live: live)
      #if os(iOS)
        store.commands = presenter.commandHost()
      #endif
      do {
        let endpoint = base.appending(path: "action")
        let transport = URLActionTransport(maxFrameBytes: store.maxFrameBytes, session: session) {
          URLRequest(url: endpoint)
        }
        let core = try await prepare(transport, store)
        let bridge = EventBridge(store: store, core: core)
        let limit = store.maxFrameBytes
        let loader = store.registerLoader {
          try await readInitial(transport.viewRequest(base.appending(path: initial)), limit: limit)
        }
        loader.onInstalled = { [weak bridge] in
          self.issue = nil
          if let bridge { onViewReady(bridge) }
        }
        store.onError = { code, message in self.issue = code + " " + message }
        store.onLiveTransportRecovered = { message in
          if self.issue == "Transport " + message { self.issue = nil }
        }
        try await loader.open()
        connection = bridge
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
