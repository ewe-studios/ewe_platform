#if os(iOS) || os(macOS)
import EweCore
import Foundation

/// Opens independent application sockets with the supplied network session.
public final class AppWebSockets: HostWebSockets {
  private let session: URLSession
  public init(session: URLSession = .shared) { self.session = session }

  public func openSocket(endpoint: String) async throws -> AppWebSocket {
    guard let url = URL(string: endpoint), let scheme = url.scheme?.lowercased(),
          ["ws", "wss"].contains(scheme), url.host != nil else {
      throw CoreError.Transport(detail: "Invalid WebSocket endpoint")
    }
    let task = session.webSocketTask(with: url)
    let opened = AsyncThrowingStream<Void, Error>.makeStream()
    let delegate = SocketHandshake(opened.continuation)
    task.delegate = delegate
    task.resume()
    do {
      var events = opened.stream.makeAsyncIterator()
      guard try await events.next() != nil else { throw URLError(.networkConnectionLost) }
      try Task.checkCancellation()
      return AppWebSocket(socket: SessionSocket(task))
    } catch {
      task.cancel()
      throw CoreError.Transport(detail: error.localizedDescription)
    }
  }
}

private final class SocketHandshake: NSObject, URLSessionWebSocketDelegate {
  let result: AsyncThrowingStream<Void, Error>.Continuation
  init(_ result: AsyncThrowingStream<Void, Error>.Continuation) { self.result = result }

  func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol name: String?) {
    result.yield(())
    result.finish()
  }
  func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
    result.finish(throwing: error ?? URLError(.networkConnectionLost))
  }
  func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask,
                  didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?) {
    result.finish()
  }
}

private final class SessionSocket: HostWebSocket {
  let task: URLSessionWebSocketTask
  init(_ task: URLSessionWebSocketTask) { self.task = task }

  func sendMessage(message: SocketMessage) async throws {
    do {
      switch message {
      case .text(let value): try await task.send(.string(value))
      case .binary(let value): try await task.send(.data(value))
      }
    } catch { throw CoreError.Transport(detail: error.localizedDescription) }
  }

  func receiveMessage() async throws -> SocketMessage? {
    do {
      switch try await task.receive() {
      case .string(let value): return .text(value: value)
      case .data(let value): return .binary(value: value)
      @unknown default: throw URLError(.cannotDecodeContentData)
      }
    } catch {
      if task.closeCode == .normalClosure { return nil }
      throw CoreError.Transport(detail: error.localizedDescription)
    }
  }

  func closeSocket() { task.cancel(with: .normalClosure, reason: nil) }
}
#endif
