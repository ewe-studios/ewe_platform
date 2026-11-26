import Foundation

/// Send and receive through one adapter registered on the manager.
@MainActor public protocol ConnectionAdapter {
  func open(endpoint: URL, events: ConnectionEvents) throws -> any Channel
}

@MainActor public struct ConnectionEvents {
  public let listening: () -> Bool
  public let opened: () -> Void
  public let context: (Data) -> Void
  public let deliver: (Data) -> Void
  public let disconnected: (String) -> Void
}

@MainActor public protocol ConnectionHandle: Channel {
  func releaseRetention()
}

@MainActor public enum ConnectionError: Error { case closed, unsupported, invalidEndpoint }

/// All connections in this manager use the authentication context of its adapters.
@MainActor public final class ConnectionManager {
  private final class Consumer {
    let owner: ObjectIdentifier?
    let accept: (Data) -> Bool
    let receive: ((Data) -> Void)?
    let onOpen: () -> Void
    let onContext: (Data) -> Void
    let onError: (String) -> Void
    init(owner: AnyObject?, accept: @escaping (Data) -> Bool, receive: ((Data) -> Void)?, onOpen: @escaping () -> Void, onContext: @escaping (Data) -> Void, onError: @escaping (String) -> Void) {
      self.owner = owner.map(ObjectIdentifier.init)
      self.accept = accept
      self.receive = receive
      self.onOpen = onOpen
      self.onContext = onContext
      self.onError = onError
    }
  }
  private struct Key: Equatable {
    let endpoint: URL
    let transport: String
    let settings: ObjectIdentifier?
  }
  private final class Entry {
    let key: Key
    let shared: Bool
    var consumers: [ObjectIdentifier: Consumer] = [:]
    var retainers: Set<UInt64> = []
    var opened = false
    var context: Data?
    var connection: (any Channel)?
    init(key: Key, shared: Bool) { self.key = key; self.shared = shared }
  }
  private final class Handle: ConnectionHandle {
    weak var manager: ConnectionManager?
    let entry: Entry
    let consumer: Consumer
    let retention: UInt64
    init(_ manager: ConnectionManager, _ entry: Entry, _ consumer: Consumer, _ retention: UInt64) {
      self.manager = manager; self.entry = entry; self.consumer = consumer; self.retention = retention
    }
    func send(_ bytes: Data) async throws {
      guard entry.consumers[ObjectIdentifier(consumer)] != nil, let channel = entry.connection else { throw ConnectionError.closed }
      try await channel.send(bytes)
    }
    func close() {
      entry.consumers.removeValue(forKey: ObjectIdentifier(consumer))
      manager?.releaseUnused(entry)
    }
    func releaseRetention() {
      entry.retainers.remove(retention)
      manager?.releaseUnused(entry)
    }
  }

  private let baseURL: URL?
  private var adapters: [String: any ConnectionAdapter] = [:]
  private var entries: [ObjectIdentifier: Entry] = [:]
  private var closed = false
  private var tag: UInt64 = 0
  public func nextTag() throws -> UInt64 {
    guard tag != .max else { throw ConnectionError.closed }
    tag += 1
    return tag
  }
  public init(baseURL: URL? = nil, adapters: [String: any ConnectionAdapter] = [:]) {
    self.baseURL = baseURL
    self.adapters = adapters
  }

  public func supports(_ transport: String) -> Bool { adapters[transport.lowercased()] != nil }

  public func register(_ transport: String, adapter: any ConnectionAdapter) throws {
    let name = transport.lowercased()
    guard !closed, adapters[name] == nil else { throw ConnectionError.closed }
    adapters[name] = adapter
  }

  public func acquire(
    _ transport: String, endpoint: URL, shared: Bool? = nil, keepAlive: Bool = false,
    settings: AnyObject? = nil, owner: AnyObject? = nil, accept: @escaping (Data) -> Bool = { _ in true },
    onOpen: @escaping () -> Void = {}, onContext: @escaping (Data) -> Void = { _ in },
    onError: @escaping (String) -> Void, receive: ((Data) -> Void)? = nil
  ) throws -> any ConnectionHandle {
    let name = transport.lowercased()
    guard !closed, let adapter = adapters[name] else { throw ConnectionError.unsupported }
    guard let url = URL(string: endpoint.relativeString, relativeTo: baseURL)?.absoluteURL,
      var parts = URLComponents(url: url, resolvingAgainstBaseURL: true),
      parts.user == nil, parts.password == nil, parts.fragment == nil else { throw ConnectionError.invalidEndpoint }
    if name == "websocket" {
      if parts.scheme == "https" { parts.scheme = "wss" }
      if parts.scheme == "http" { parts.scheme = "ws" }
    }
    guard let address = parts.url else { throw ConnectionError.invalidEndpoint }
    let key = Key(endpoint: address, transport: name, settings: settings.map(ObjectIdentifier.init))
    let share = shared ?? (name == "websocket")
    let previous = share ? entries.values.first { $0.shared && $0.key == key } : nil
    let entry = previous ?? Entry(key: key, shared: share)
    let consumer = Consumer(owner: owner, accept: accept, receive: receive, onOpen: onOpen, onContext: onContext, onError: onError)
    let id = ObjectIdentifier(consumer)
    entry.consumers[id] = consumer
    let retention = keepAlive ? try nextTag() : 0
    if keepAlive { entry.retainers.insert(retention) }
    if previous == nil {
      let eid = ObjectIdentifier(entry)
      entries[eid] = entry
      do {
        let channel = try adapter.open(endpoint: address, events: ConnectionEvents(
          listening: { [weak entry] in entry?.consumers.values.contains { $0.receive != nil } ?? false },
          opened: { [weak entry] in
            guard let entry else { return }
            entry.opened = true
            for consumer in Array(entry.consumers.values) { consumer.onOpen() }
          },
          context: { [weak entry] bytes in
            guard let entry else { return }
            entry.context = bytes
            for consumer in Array(entry.consumers.values) { consumer.onContext(bytes) }
          },
          deliver: { [weak entry] bytes in
            guard let entry else { return }
            var owners = Set<ObjectIdentifier>()
            for (token, consumer) in Array(entry.consumers) {
              guard entry.consumers[token] != nil, let receive = consumer.receive,
                consumer.accept(bytes), consumer.owner.map({ !owners.contains($0) }) ?? true else { continue }
              if let owner = consumer.owner { owners.insert(owner) }
              receive(bytes)
            }
          },
          disconnected: { [weak entry] cause in
            guard let entry else { return }
            entry.opened = false
            entry.context = nil
            for consumer in Array(entry.consumers.values) { consumer.onError(cause) }
          }))
        if entries[eid] != nil { entry.connection = channel } else { channel.close() }
      } catch { entries.removeValue(forKey: eid); throw error }
    }
    if previous != nil {
      Task { @MainActor [weak entry] in
        guard let entry, entry.consumers[id] != nil else { return }
        if entry.opened { onOpen() }
        if let bytes = entry.context { onContext(bytes) }
      }
    }
    return Handle(self, entry, consumer, retention)
  }

  private func releaseUnused(_ entry: Entry) {
    if entry.consumers.isEmpty && entry.retainers.isEmpty,
      entries.removeValue(forKey: ObjectIdentifier(entry)) != nil { entry.connection?.close() }
  }

  public func close() {
    closed = true
    let active = Array(entries.values)
    entries.removeAll()
    for entry in active {
      entry.consumers.removeAll(); entry.retainers.removeAll(); entry.connection?.close()
    }
  }
}
