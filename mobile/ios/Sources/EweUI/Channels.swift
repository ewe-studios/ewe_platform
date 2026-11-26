import Foundation

enum ChannelError: Error {
  case httpStatus(Int)
  case invalidFrame, frameTooLarge, unsupportedTransport
}

@MainActor public final class URLSessionChannelFactory: ChannelFactory {
  let session: URLSession
  let limit: Int
  let reconnectDelay: Duration
  let idleTimeout: TimeInterval
  let request: (URL) -> URLRequest
  let baseURL: URL?
  public init(
    session: URLSession = .shared, baseURL: URL? = nil, maxFrameBytes: Int = 4 * 1024 * 1024,
    idleTimeout: TimeInterval = 300, reconnectDelay: Duration = .seconds(1),
    request: @escaping (URL) -> URLRequest = { URLRequest(url: $0) }
  ) {
    self.idleTimeout = idleTimeout
    self.reconnectDelay = reconnectDelay
    self.session = session
    self.baseURL = baseURL
    self.limit = max(1, maxFrameBytes)
    self.request = request
  }
  public func connectionManager() -> ConnectionManager {
    ConnectionManager(baseURL: baseURL, adapters: [
      "http": ManagedAdapter(factory: self, transport: "http"),
      "sse": ManagedAdapter(factory: self, transport: "sse"),
      "websocket": ManagedAdapter(factory: self, transport: "websocket"),
    ])
  }
  private struct ManagedAdapter: ConnectionAdapter {
    let factory: URLSessionChannelFactory
    let transport: String
    func open(endpoint: URL, events: ConnectionEvents) throws -> any Channel {
      let shared = ManagedChannel()
      func connect() {
        guard !shared.closed else { return }
        shared.active = factory.openChannel(endpoint: endpoint, transport: transport, cursor: nil,
          listening: events.listening, onFrame: { bytes in
            if transport != "websocket", SubscriptionFrames.caps(bytes) != nil { events.context(bytes) }
            else { events.deliver(bytes) }
          }, onError: { [weak shared] cause in
            guard let shared, !shared.closed else { return }
            events.disconnected(cause)
            shared.retry = Task {
              do { try await Task.sleep(for: factory.reconnectDelay); connect() }
              catch { return }
            }
          }, onOpen: events.opened)
      }
      connect()
      return shared
    }
  }
  public func open(
    endpoint: URL, transport: String, streamId: UInt64, resumeFrom: UInt64,
    onFrame: @escaping @MainActor (Data) -> Void, onError: @escaping @MainActor (String) -> Void
  ) -> any Channel {
    openChannel(endpoint: endpoint, transport: transport, cursor: (streamId, resumeFrom),
      listening: { true }, onFrame: onFrame, onError: onError)
  }
  private func openChannel(
    endpoint: URL, transport: String, cursor: (UInt64, UInt64)?, listening: @escaping @MainActor () -> Bool,
    onFrame: @escaping @MainActor (Data) -> Void, onError: @escaping @MainActor (String) -> Void, onOpen: (() -> Void)? = nil
  ) -> any Channel {
    let channel = URLChannel(canSend: transport.lowercased() == "websocket")
    channel.work = Task {
      do {
        let resolved =
          URL(string: endpoint.relativeString, relativeTo: baseURL)?.absoluteURL ?? endpoint
        guard var parts = URLComponents(url: resolved, resolvingAgainstBaseURL: true) else {
          throw ChannelError.invalidFrame
        }
        if let (stream, after) = cursor {
          var query = (parts.queryItems ?? []).filter { !["stream_id", "resume_from"].contains($0.name) }
          query += [URLQueryItem(name: "stream_id", value: String(stream)), URLQueryItem(name: "resume_from", value: String(after))]
          parts.queryItems = query
        }
        if transport.lowercased() == "websocket" {
          if parts.scheme == "http" { parts.scheme = "ws" }
          if parts.scheme == "https" { parts.scheme = "wss" }
        }
        guard let url = parts.url else { throw ChannelError.invalidFrame }
        var req = request(url)
        req.timeoutInterval = idleTimeout
        switch transport.lowercased() {
        case "websocket":
          let socket = session.webSocketTask(with: req)
          channel.setSocket(socket)
          socket.maximumMessageSize = limit
          socket.resume()
          onOpen?()
          while !Task.isCancelled {
            guard case .data(let data) = try await socket.receive() else {
              throw ChannelError.invalidFrame
            }
            guard data.count <= limit else { throw ChannelError.frameTooLarge }
            if listening() { onFrame(data) }
          }
        case "sse", "http":
          if cursor == nil {
            req.setValue(transport.lowercased() == "sse" ? "text/event-stream" : "application/primal-frame", forHTTPHeaderField: "Accept")
            let header = SubscriptionFrames.hello(0).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
            req.setValue(header, forHTTPHeaderField: "X-Ewe-Hello")
            channel.control = { [session, request] data in
              var upstream = request(url)
              upstream.httpMethod = "POST"
              upstream.setValue("application/primal-frame", forHTTPHeaderField: "Content-Type")
              upstream.httpBody = data
              let (_, response) = try await session.bytes(for: upstream)
              guard let response = response as? HTTPURLResponse else { throw URLError(.badServerResponse) }
              guard (200..<300).contains(response.statusCode) else { throw ChannelError.httpStatus(response.statusCode) }
            }
          }
          let (bytes, response) = try await session.bytes(for: req)
          guard let http = response as? HTTPURLResponse else { throw ChannelError.invalidFrame }
          if http.statusCode == 410, let (stream, after) = cursor {
            onFrame(SubscriptionFrames.expired(stream:stream, after:after))
            return
          }
          guard (200..<300).contains(http.statusCode) else {
            throw ChannelError.httpStatus(http.statusCode)
          }
          onOpen?()
          if transport.lowercased() == "sse" {
            try await readEvents(bytes, listening, onFrame)
            if !Task.isCancelled { throw URLError(.networkConnectionLost) }
          } else {
            try await readFrames(bytes) { frame in if listening() { onFrame(frame) } }
            if cursor == nil && !Task.isCancelled { throw URLError(.networkConnectionLost) }
          }
        default: throw ChannelError.unsupportedTransport
        }
      } catch { channel.fail(error); if !Task.isCancelled { onError(String(describing: error)) } }
    }
    return channel
  }
  private func readFrames(_ bytes: URLSession.AsyncBytes, _ onFrame: (Data) -> Void) async throws {
    var builder = LengthPrefixedFrames(limit: limit)
    for try await byte in bytes { if let frame = try builder.next(byte) { onFrame(frame) } }
    try builder.finish()
  }
  private func readEvents(_ bytes: URLSession.AsyncBytes, _ listening: () -> Bool, _ onFrame: (Data) -> Void) async throws {
    var line = Data()
    var payload = Data()
    let encodedLimit = limit / 3 * 4 + 8
    for try await byte in bytes {
      if byte == 13 { continue }
      if byte != 10 {
        guard line.count <= encodedLimit + 6 else { throw ChannelError.frameTooLarge }
        line.append(byte)
        continue
      }
      if line.isEmpty && !payload.isEmpty {
        guard listening() else { payload.removeAll(keepingCapacity: true); continue }
        guard let frame = Data(base64Encoded: payload), frame.count <= limit else {
          throw ChannelError.invalidFrame
        }
        onFrame(frame)
        payload.removeAll(keepingCapacity: true)
      } else if line.starts(with: "data:".utf8) {
        var chunk = line.dropFirst(5)
        if chunk.first == 32 { chunk = chunk.dropFirst() }
        guard chunk.count <= encodedLimit - payload.count else { throw ChannelError.frameTooLarge }
        payload.append(contentsOf: chunk)
      }
      line.removeAll(keepingCapacity: true)
    }
    guard line.isEmpty && payload.isEmpty else { throw ChannelError.invalidFrame }
  }
}
@MainActor private final class ManagedChannel: Channel {
  var active: (any Channel)?
  var retry: Task<Void, Never>?
  var closed = false
  func send(_ data: Data) async throws {
    guard !closed, let active else { throw ConnectionError.closed }
    try await active.send(data)
  }
  func close() { closed = true; retry?.cancel(); active?.close(); active = nil }
}

@MainActor private final class URLChannel: Channel {
  var control: ((Data) async throws -> Void)?
  private let canSend: Bool
  init(canSend: Bool) { self.canSend = canSend }
  var work: Task<Void, Never>?
  private var socket: URLSessionWebSocketTask?
  private var failure: Error?
  private var waiting: [CheckedContinuation<URLSessionWebSocketTask, Error>] = []
  func setSocket(_ value: URLSessionWebSocketTask) {
    socket = value
    let pending = waiting
    waiting.removeAll()
    pending.forEach { $0.resume(returning: value) }
  }
  func fail(_ error: Error) {
    failure = error
    let pending = waiting
    waiting.removeAll()
    pending.forEach { $0.resume(throwing: error) }
  }
  func send(_ bytes: Data) async throws {
    if let control { try await control(bytes); return }
    guard canSend else { throw ConnectionError.unsupported }
    if let failure { throw failure }
    let ready: URLSessionWebSocketTask
    if let socket { ready = socket } else {
      ready = try await withCheckedThrowingContinuation { waiting.append($0) }
    }
    try await ready.send(.data(bytes))
  }
  func close() {
    fail(ConnectionError.closed)
    work?.cancel()
    socket?.cancel(with: .goingAway, reason: nil)
    work = nil
    socket = nil
  }
}
struct LengthPrefixedFrames {
  let limit: Int
  var lastLength: Int?
  var bytes = Data()
  init(limit: Int) { self.limit = limit }
  mutating func next(_ byte: UInt8) throws -> Data? {
    bytes.append(byte)
    if let length = lastLength {
      if bytes.count == length {
        let result = bytes
        bytes = Data()
        lastLength = nil
        return result
      }
    } else if bytes.count == 4 {
      let length = bytes.enumerated().reduce(0) { $0 | (Int($1.element) << ($1.offset * 8)) }
      guard length > 0, length <= limit else { throw ChannelError.frameTooLarge }
      lastLength = length
      bytes = Data()
    }
    return nil
  }
  func finish() throws {
    if lastLength != nil || !bytes.isEmpty { throw ChannelError.invalidFrame }
  }
}
