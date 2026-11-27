#if os(iOS) || os(macOS)
import EweCore
import EweWire
import FlatBuffers
import Foundation

/// Wait for the reply to each action request on this screen.
@MainActor public final class SocketActionConnection: HostTransport {
  private struct Receipt: Hashable, Sendable {
    let stream: UInt64
    let sequence: UInt64
  }

  private enum Reply: Hashable, Sendable {
    case capabilities
    case action(Receipt)
  }

  public let requestTag: UInt64
  private var hello: Data?
  private var reconnect = false
  private var stream: UInt64?
  private var handle: (any ConnectionHandle)?
  private var waiting: [Reply: CheckedContinuation<Data, Error>] = [:]
  private var waitingForSession: [UInt64: CheckedContinuation<Void, Error>] = [:]
  private var readinessTag: UInt64 = 0
  private let handshakeTimeout: Duration
  private let actionTimeout: Duration
  private let onReply: (Data) -> Void
  private let onError: (String) -> Void

  public init(
    manager: ConnectionManager, endpoint: URL, handshakeTimeout: Duration = .seconds(60),
    actionTimeout: Duration? = nil, onReply: @escaping (Data) -> Void,
    onError: @escaping (String) -> Void
  ) throws {
    self.requestTag = try manager.nextTag()
    self.handshakeTimeout = handshakeTimeout
    self.actionTimeout = actionTimeout ?? handshakeTimeout
    self.onReply = onReply
    self.onError = onError
    handle = try manager.acquire("websocket", endpoint: endpoint, accept: { [weak self] in
      self?.accepts($0) ?? false
    }, onOpen: { [weak self] in
      self?.negotiateAgain()
    }, onError: { [weak self] cause in
      self?.disconnected(cause)
    }, receive: { [weak self] in
      self?.receive($0)
    })
  }

  private func packet(_ data: Data) throws -> ewe_ui_Frame {
    var buffer = ByteBuffer(data: data)
    return try getCheckedRoot(byteBuffer: &buffer, fileId: "EWF1")
  }

  private func accepts(_ bytes: Data) -> Bool {
    guard let frame = try? packet(bytes), frame.kindType == .framemessage,
      let body = frame.kind(type: ewe_ui_FrameMessage.self) else { return false }
    if body.messagesCount > 0, let caps = body.messages(at: 0)?.control?.caps {
      return caps.requestTag == requestTag
    }
    return body.ack != 0 && stream == frame.streamId
  }

  private func negotiateAgain() {
    guard reconnect, let hello, let handle else { return }
    reconnect = false
    Task {
      do { try await handle.send(hello) }
      catch { disconnected(String(describing: error)) }
    }
  }

  private func disconnected(_ cause: String) {
    guard handle != nil else { return }
    reconnect = true
    stream = nil
    failReplies(CoreError.Transport(detail: cause))
    onError(cause)
  }

  private func receive(_ bytes: Data) {
    guard let frame = try? packet(bytes), let body = frame.kind(type: ewe_ui_FrameMessage.self)
    else { return }
    if body.messagesCount > 0, let calls = body.messages(at: 0)?.control?.caps {
      let initial = waiting.removeValue(forKey: .capabilities)
      if initial == nil { onReply(bytes) }
      guard handle != nil else {
        initial?.resume(throwing: ConnectionError.closed)
        return
      }
      stream = calls.status == .ok ? calls.streamId : nil
      if stream != nil { finishReadiness(.success(())) }
      else { finishReadiness(.failure(ConnectionError.invalidEndpoint)) }
      initial?.resume(returning: bytes)
      return
    }
    let receipt = Reply.action(Receipt(stream: frame.streamId, sequence: body.ack))
    if let reply = waiting.removeValue(forKey: receipt) { reply.resume(returning: bytes) }
    else { onReply(bytes) }
  }

  private func finish(_ key: Reply, _ result: Result<Data, Error>) {
    waiting.removeValue(forKey: key)?.resume(with: result)
  }

  private func failReplies(_ error: Error) {
    let pending = Array(waiting.values)
    waiting.removeAll()
    for reply in pending { reply.resume(throwing: error) }
  }

  private func finishReadiness(_ result: Result<Void, Error>) {
    let pending = Array(waitingForSession.values)
    waitingForSession.removeAll()
    for waiter in pending { waiter.resume(with: result) }
  }

  func awaitReady() async throws {
    guard handle != nil else { throw ConnectionError.closed }
    if stream != nil { return }
    guard readinessTag < .max else { throw ConnectionError.closed }
    readinessTag += 1
    let tag = readinessTag
    let deadline = Task {
      try await Task.sleep(for: handshakeTimeout)
      waitingForSession.removeValue(forKey: tag)?.resume(throwing: URLError(.timedOut))
    }
    defer { deadline.cancel() }
    try await withTaskCancellationHandler {
      try Task.checkCancellation()
      try await withCheckedThrowingContinuation { waitingForSession[tag] = $0 }
    } onCancel: {
      Task { @MainActor in
        self.waitingForSession.removeValue(forKey: tag)?.resume(throwing: CancellationError())
      }
    }
    guard stream != nil else { throw ConnectionError.closed }
  }

  public func send(request: Data) async throws -> Data? {
    let frame = try packet(request)
    guard let handle, frame.kindType == .framerequest,
      let call = frame.kind(type: ewe_ui_FrameRequest.self) else { throw ConnectionError.closed }
    if let hello = call.hello {
      guard waiting[.capabilities] == nil, hello.requestTag == requestTag
      else { throw ConnectionError.invalidEndpoint }
      self.hello = request
      return try await send(request, through: handle, awaiting: .capabilities, timeout: handshakeTimeout)
    }
    guard stream == frame.streamId else { throw ConnectionError.closed }
    let receipt = Reply.action(Receipt(stream: frame.streamId, sequence: frame.sequenceId))
    guard frame.sequenceId != 0, waiting[receipt] == nil else { throw ConnectionError.invalidEndpoint }
    return try await send(request, through: handle, awaiting: receipt, timeout: actionTimeout)
  }

  private func send(
    _ request: Data, through handle: any ConnectionHandle, awaiting reply: Reply, timeout: Duration
  ) async throws -> Data {
    let deadline = Task {
      try await Task.sleep(for: timeout)
      finish(reply, .failure(URLError(.timedOut)))
    }
    defer { deadline.cancel() }
    return try await withTaskCancellationHandler {
      try Task.checkCancellation()
      return try await withCheckedThrowingContinuation { continuation in
        waiting[reply] = continuation
        Task {
          guard waiting[reply] != nil else { return }
          do { try await handle.send(request) }
          catch { finish(reply, .failure(error)) }
        }
      }
    } onCancel: { Task { @MainActor in self.finish(reply, .failure(CancellationError())) } }
  }

  public func close() {
    handle?.close()
    handle = nil
    stream = nil
    failReplies(ConnectionError.closed)
    finishReadiness(.failure(ConnectionError.closed))
  }
}
#endif
