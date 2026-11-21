#if os(iOS) || os(macOS)
  import EweCore
  import Foundation

  /// Connects the generated native core to a screen and its local action references.
  @MainActor public final class NativeActionCore: ActionCore {
    public let raw: EweCore.NativeCore

    public init(_ core: EweCore.NativeCore, store: NodeStore) {
      raw = core
      raw.setFrameListener(listener: FrameDelivery(store))
    }

    public func trigger(reference: String, event: EventInput) async throws -> Data? {
      try await raw.trigger(
        reference: reference,
        event: EweCore.EventInput(
          nodeId: event.nodeId, eventName: event.eventName, value: event.value,
          checked: event.checked, parentNode: event.parentNode, state: event.state))
    }

    public func applyCapsFrame(_ frame: Data) throws { try raw.applyCapsFrame(frame: frame) }

    deinit { raw.clearFrameListener() }
  }

  private final class FrameDelivery: EweCore.FrameListener, @unchecked Sendable {
    private weak var store: NodeStore?
    @MainActor init(_ store: NodeStore) { self.store = store }
    func onFrame(frame: Data) {
      DispatchQueue.main.async { [weak store] in store?.receive(frame) }
    }
  }

  /// Sends one action request and reads one bounded, length-prefixed response.
  public final class URLActionTransport: EweCore.HostTransport, @unchecked Sendable {
    private let request: @Sendable () -> URLRequest
    private let session: URLSession
    private let limit: Int
    private let lock = NSLock()
    private var handshake = ""

    public init(
      maxFrameBytes: Int = 4 * 1024 * 1024, session: URLSession = .shared,
      request: @escaping @Sendable () -> URLRequest
    ) {
      self.limit = maxFrameBytes
      self.session = session
      self.request = request
    }

    public func configure(hello: Data) {
      lock.withLock {
        handshake = hello.base64EncodedString().replacingOccurrences(of: "+", with: "-")
          .replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
      }
    }

    func viewRequest(_ url: URL) -> URLRequest {
      var req = URLRequest(url: url)
      req.setValue(lock.withLock { handshake }, forHTTPHeaderField: "X-Ewe-Hello")
      req.setValue("application/vnd.ewe.frame", forHTTPHeaderField: "Accept")
      return req
    }

    public func send(request bytes: Data) async throws -> Data? {
      var req = request()
      req.httpMethod = "POST"
      req.httpBody = bytes
      req.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
      req.setValue(lock.withLock { handshake }, forHTTPHeaderField: "X-Ewe-Hello")
      let (stream, response) = try await session.bytes(for: req)
      guard let status = (response as? HTTPURLResponse)?.statusCode, (200..<300).contains(status)
      else {
        throw CoreError.Transport(
          detail:
            "Action request failed with HTTP status \((response as? HTTPURLResponse)?.statusCode ?? 0)"
        )
      }
      if status == 204 { return nil }
      var parser = LengthPrefixedFrames(limit: limit)
      var reply: Data?
      for try await byte in stream {
        if reply != nil { throw CoreError.Transport(detail: "Expected one response frame") }
        reply = try parser.next(byte)
      }
      try parser.finish()
      guard let reply else { throw CoreError.Transport(detail: "Missing response frame") }
      return reply
    }
  }

  @MainActor extension NativeActionCore {
    /// Opens a remote-action screen with the server's capability frame.
    public static func remote(names: [String], store: NodeStore, transport: URLActionTransport)
      async throws -> NativeActionCore
    {
      let core = try EweCore.NativeCore.remote(platform: .ios, names: names, transport: transport)
      let adapter = NativeActionCore(core, store: store)
      try await adapter.negotiate(transport)
      return adapter
    }

    public func negotiate(_ transport: URLActionTransport) async throws {
      let hello = raw.hello()
      transport.configure(hello: hello)
      guard let caps = try await transport.send(request: hello) else {
        throw CoreError.Transport(detail: "Missing server capabilities")
      }
      try applyCapsFrame(caps)
    }
  }
#endif
