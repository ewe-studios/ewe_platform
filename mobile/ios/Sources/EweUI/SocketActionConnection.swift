#if os(iOS) || os(macOS)
import EweCore
import EweWire
import FlatBuffers
import Foundation

/// The socket carries one outbound request and a later reply for this screen.
@MainActor public final class SocketActionConnection: HostTransport {
  public let requestTag: UInt64
  private var hello: Data?
  private var reconnect = false
  private var stream: UInt64?
  private var handle: (any ConnectionHandle)?
  private var waiting: CheckedContinuation<Data, Error>?
  private let timeout: Duration
  private let onReply: (Data) -> Void
  private let onError: (String) -> Void

  public init(manager: ConnectionManager, endpoint: URL, handshakeTimeout: Duration = .seconds(60),
    onReply: @escaping (Data) -> Void, onError: @escaping (String) -> Void) throws {
    self.requestTag = try manager.nextTag()
    self.timeout = handshakeTimeout
    self.onReply = onReply
    self.onError = onError
    handle = try manager.acquire("websocket", endpoint: endpoint, accept: { [weak self] in self?.accepts($0) ?? false },
      onOpen: { [weak self] in
        guard let self, reconnect, let hello else { return }
        reconnect = false
        Task { @MainActor in
          do { try await self.handle?.send(hello) } catch { self.onError(String(describing: error)) }
        }
      }, onError: { [weak self] cause in
        self?.reconnect = true
        self?.stream = nil
        self?.finish(.failure(CoreError.Transport(detail: cause)))
        self?.onError(cause)
      }, receive: { [weak self] in self?.receive($0) })
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

  private func receive(_ bytes: Data) {
    guard let frame = try? packet(bytes), let body = frame.kind(type: ewe_ui_FrameMessage.self) else { return }
    if body.messagesCount > 0, let caps = body.messages(at: 0)?.control?.caps {
      stream = caps.status == .ok ? caps.streamId : nil
      if waiting == nil { onReply(bytes) } else { finish(.success(bytes)) }
    } else { onReply(bytes) }
  }

  private func finish(_ result: Result<Data, Error>) {
    let reply = waiting
    waiting = nil
    reply?.resume(with: result)
  }

  public func send(request: Data) async throws -> Data? {
    let frame = try packet(request)
    guard let handle, frame.kindType == .framerequest,
      let call = frame.kind(type: ewe_ui_FrameRequest.self) else { throw ConnectionError.closed }
    guard let hello = call.hello else {
      guard stream == frame.streamId else { throw ConnectionError.closed }
      try await handle.send(request)
      return nil
    }
    guard waiting == nil, hello.requestTag == requestTag else { throw ConnectionError.invalidEndpoint }
    self.hello = request
    let deadline = Task {
      try await Task.sleep(for: timeout)
      finish(.failure(URLError(.timedOut)))
    }
    defer { deadline.cancel() }
    return try await withTaskCancellationHandler {
      try await withCheckedThrowingContinuation { reply in
        waiting = reply
        Task {
          do { try await handle.send(request) } catch { finish(.failure(error)) }
        }
      }
    } onCancel: { Task { @MainActor in self.finish(.failure(CancellationError())) } }
  }

  public func close() {
    finish(.failure(ConnectionError.closed))
    handle?.close()
    handle = nil
    stream = nil
  }
}
#endif
