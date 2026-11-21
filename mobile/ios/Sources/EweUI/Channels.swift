import Foundation

private enum ChannelError: Error {
  case httpStatus(Int)
  case invalidFrame, frameTooLarge, unsupportedTransport
}

@MainActor public final class URLSessionChannelFactory: ChannelFactory {
  let session: URLSession
  let limit: Int
  let idleTimeout: TimeInterval
  let request: (URL) -> URLRequest
  let baseURL: URL?
  public init(
    session: URLSession = .shared, baseURL: URL? = nil, maxFrameBytes: Int = 4 * 1024 * 1024,
    idleTimeout: TimeInterval = 300,
    request: @escaping (URL) -> URLRequest = { URLRequest(url: $0) }
  ) {
    self.idleTimeout = idleTimeout
    self.session = session
    self.baseURL = baseURL
    self.limit = max(1, maxFrameBytes)
    self.request = request
  }
  public func open(
    endpoint: URL, transport: String, streamId: UInt64, resumeFrom: UInt64,
    onFrame: @escaping @MainActor (Data) -> Void, onError: @escaping @MainActor (String) -> Void
  ) -> any Channel {
    let channel = URLChannel()
    channel.work = Task {
      do {
        let resolved =
          URL(string: endpoint.relativeString, relativeTo: baseURL)?.absoluteURL ?? endpoint
        guard var parts = URLComponents(url: resolved, resolvingAgainstBaseURL: true) else {
          throw ChannelError.invalidFrame
        }
        var query = (parts.queryItems ?? []).filter {
          !["stream_id", "resume_from"].contains($0.name)
        }
        query += [
          URLQueryItem(name: "stream_id", value: String(streamId)),
          URLQueryItem(name: "resume_from", value: String(resumeFrom)),
        ]
        parts.queryItems = query
        guard let url = parts.url else { throw ChannelError.invalidFrame }
        var req = request(url)
        req.timeoutInterval = idleTimeout
        switch transport.lowercased() {
        case "websocket":
          let socket = session.webSocketTask(with: req)
          channel.socket = socket
          socket.maximumMessageSize = limit
          socket.resume()
          while !Task.isCancelled {
            guard case .data(let data) = try await socket.receive() else {
              throw ChannelError.invalidFrame
            }
            guard data.count <= limit else { throw ChannelError.frameTooLarge }
            onFrame(data)
          }
        case "sse", "http":
          let (bytes, response) = try await session.bytes(for: req)
          guard let http = response as? HTTPURLResponse else { throw ChannelError.invalidFrame }
          guard (200..<300).contains(http.statusCode) else {
            throw ChannelError.httpStatus(http.statusCode)
          }
          if transport.lowercased() == "sse" {
            try await readEvents(bytes, onFrame)
          } else {
            try await readFrames(bytes, onFrame)
          }
        default: throw ChannelError.unsupportedTransport
        }
      } catch { if !Task.isCancelled { onError(String(describing: error)) } }
    }
    return channel
  }
  private func readFrames(_ bytes: URLSession.AsyncBytes, _ onFrame: (Data) -> Void) async throws {
    var builder = LengthPrefixedFrames(limit: limit)
    for try await byte in bytes { if let frame = try builder.next(byte) { onFrame(frame) } }
    try builder.finish()
  }
  private func readEvents(_ bytes: URLSession.AsyncBytes, _ onFrame: (Data) -> Void) async throws {
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
@MainActor private final class URLChannel: Channel {
  var work: Task<Void, Never>?
  var socket: URLSessionWebSocketTask?
  func close() {
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
