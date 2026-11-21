#if os(iOS) || os(macOS)
import EweCore
import Foundation

/// Uses one application session and a separate task for each RPC exchange.
public final class URLSessionRpcClient: HostHttpClient, @unchecked Sendable {
  private let session: URLSession
  private let lock = NSLock()
  private var last: UInt64 = 0
  private var exchanges: [UInt64: RPCExchange] = [:]

  public init(session: URLSession = .shared) { self.session = session }

  public func newExchange(method: String, url: String, headers: [HttpHeader]) throws -> UInt64 {
    guard let address = URL(string: url) else { throw CoreError.Transport(detail: "Invalid RPC address") }
    var request = URLRequest(url: address)
    request.httpMethod = method
    for header in headers { request.addValue(header.value, forHTTPHeaderField: header.name) }
    return try lock.withLock {
      guard last < UInt64.max else { throw CoreError.Transport(detail: "No more exchange identifiers") }
      last += 1
      exchanges[last] = RPCExchange(session: session, request: request)
      return last
    }
  }

  private func exchange(_ handle: UInt64) throws -> RPCExchange {
    guard let exchange = lock.withLock({ exchanges[handle] }) else {
      throw CoreError.Transport(detail: "RPC exchange closed")
    }
    return exchange
  }

  public func start(handle: UInt64, body: Data) async throws -> ResponseHead {
    try await exchange(handle).start(body)
  }

  public func read(handle: UInt64, maxBytes: UInt32) async throws -> Data? {
    try await exchange(handle).read(Int(maxBytes))
  }

  public func close(handle: UInt64) {
    lock.withLock { exchanges.removeValue(forKey: handle) }?.close()
  }
}

/// The serial queue owns the task, its buffered chunk and the waiting continuations.
private final class RPCExchange: NSObject, URLSessionDataDelegate, @unchecked Sendable {
  let session: URLSession
  var request: URLRequest
  let queue = DispatchQueue(label: "ewe.rpc.exchange")
  var task: URLSessionDataTask?
  var head: CheckedContinuation<ResponseHead, Error>?
  var reader: (Int, CheckedContinuation<Data?, Error>)?
  var bytes = Data()
  var paused = false
  var finished = false
  var issue: CoreError?

  init(session: URLSession, request: URLRequest) { self.session = session; self.request = request }

  func start(_ body: Data) async throws -> ResponseHead {
    try await withCheckedThrowingContinuation { pending in
      queue.async {
        guard !self.finished && self.task == nil else {
          pending.resume(throwing: CoreError.Transport(detail: "RPC exchange unavailable"))
          return
        }
        self.request.httpBody = body
        self.head = pending
        let task = self.session.dataTask(with: self.request)
        task.delegate = self
        self.task = task
        task.resume()
      }
    }
  }

  func read(_ limit: Int) async throws -> Data? {
    try await withCheckedThrowingContinuation { pending in
      queue.async {
        guard limit > 0 && self.reader == nil else {
          pending.resume(throwing: CoreError.Transport(detail: "Invalid RPC read"))
          return
        }
        self.reader = (limit, pending)
        self.deliver()
      }
    }
  }

  private func deliver() {
    guard let (limit, pending) = reader else { return }
    if !bytes.isEmpty {
      let chunk = bytes.prefix(limit)
      bytes.removeFirst(chunk.count)
      reader = nil
      pending.resume(returning: Data(chunk))
    } else if finished {
      reader = nil
      if let issue { pending.resume(throwing: issue) } else { pending.resume(returning: nil) }
    } else if paused {
      paused = false
      task?.resume()
    }
  }

  func urlSession(_ session: URLSession, dataTask: URLSessionDataTask,
                  didReceive response: URLResponse,
                  completionHandler: @escaping @Sendable (URLSession.ResponseDisposition) -> Void) {
    queue.async {
      guard let http = response as? HTTPURLResponse, let pending = self.head else {
        completionHandler(.cancel)
        return
      }
      self.head = nil
      // URLSession delivers decoded content to this delegate.
      let decoded = http.value(forHTTPHeaderField: "Content-Encoding") != nil
      let headers = http.allHeaderFields.flatMap { key, value -> [HttpHeader] in
        let name = String(describing: key).lowercased()
        if decoded && (name == "content-encoding" || name == "content-length") { return [] }
        return (value as? [String] ?? [String(describing: value)]).map {
          HttpHeader(name: String(describing: key), value: $0)
        }
      }
      pending.resume(returning: ResponseHead(status: UInt16(http.statusCode), headers: headers))
      completionHandler(.allow)
    }
  }

  func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
    queue.async {
      guard !self.finished else { return }
      if !self.paused { self.paused = true; dataTask.suspend() }
      self.bytes.append(data)
      self.deliver()
    }
  }

  func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
    queue.async { self.finish(error.map { CoreError.Transport(detail: $0.localizedDescription) }) }
  }

  private func finish(_ error: CoreError?) {
    guard !finished else { return }
    finished = true
    issue = error
    head?.resume(throwing: error ?? CoreError.Transport(detail: "Missing HTTP response"))
    head = nil
    task = nil
    deliver()
  }

  func close() {
    queue.async {
      self.task?.cancel()
      self.bytes.removeAll()
      self.finish(CoreError.Transport(detail: "RPC exchange closed"))
    }
  }
}
#endif

#if os(iOS) || os(macOS)
/// Adapts the generated Connect sender to the screen's action interface.
public final class ConnectActionSender: HostTransport, @unchecked Sendable {
  private let sender: ConnectSender

  public init(base: URL, client: HostHttpClient, maxFrameBytes: UInt32, chunkBytes: UInt32 = 16 * 1024) throws {
    self.sender = try ConnectSender(client: NativeHttpClient(client: client),
      baseUrl: base.absoluteString, platform: .ios, maxFrameBytes: maxFrameBytes, chunkBytes: chunkBytes)
  }

  public func send(request: Data) async throws -> Data? {
    try await sender.exchange(request: request)
  }
}
#endif
