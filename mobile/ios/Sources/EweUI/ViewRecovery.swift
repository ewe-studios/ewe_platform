import EweWire
import Foundation

public enum ViewRecoveryError: Error { case invalidSnapshot, missingCursor, attemptLimit, disposed }

/// One mounted view retains its loader, its stream positions, and its pending task.
@MainActor public final class ViewRecovery {
  private weak var store: NodeStore?
  private let load: @MainActor () async throws -> Data
  private let maxAttempts: Int
  private var flight: Task<Void, Error>?
  private var required = Set<UInt64>()
  private var attempts = 0
  private var closed = false
  private var ready = false
  public var onInstalled: (() -> Void)?
  public private(set) var generation: UInt64 = 0

  init(store: NodeStore, maxAttempts: Int, load: @escaping @MainActor () async throws -> Data) {
    precondition(maxAttempts > 0)
    self.store = store
    self.maxAttempts = maxAttempts
    self.load = load
  }

  public func open() async throws { try await run() }

  public func expire(stream: UInt64) async throws {
    guard !closed else { throw ViewRecoveryError.disposed }
    required.insert(stream)
    if let flight { return try await flight.value }
    guard attempts < maxAttempts else { ready = false; throw ViewRecoveryError.attemptLimit }
    attempts += 1
    try await run()
  }

  public func retry() async throws {
    if let flight { return try await flight.value }
    attempts = 0
    try await run()
  }

  public func accepts(_ generation: UInt64) -> Bool {
    !closed && ready && self.generation == generation
  }

  private func run() async throws {
    guard !closed else { throw ViewRecoveryError.disposed }
    if let flight { return try await flight.value }
    generation += 1
    let current = generation
    ready = false
    let task = Task { @MainActor in
      let bytes = try await load()
      try Task.checkCancellation()
      guard let store, !closed, current == generation else { throw ViewRecoveryError.disposed }
      do {
        ready = true
        try store.installSnapshot(bytes, required: required)
        required.removeAll()
        onInstalled?()
      } catch { ready = false; throw error }
    }
    flight = task
    defer { if generation == current { flight = nil } }
    try await task.value
  }

  public func close() {
    closed = true
    ready = false
    generation += 1
    flight?.cancel()
    flight = nil
    required.removeAll()
  }
}

@MainActor extension NodeStore {
  /// Use the same loader for initial content, automatic recovery, and an explicit retry.
  public func registerLoader(maxAttempts: Int = 3, load: @escaping @MainActor () async throws -> Data) -> ViewRecovery {
    recovery?.close()
    let result = ViewRecovery(store: self, maxAttempts: maxAttempts, load: load)
    recovery = result
    onSnapshot = { [weak self, weak result] _, stream in
      Task { @MainActor in
        do { try await result?.expire(stream:stream) }
        catch { if !(error is CancellationError) { self?.error("ViewRecovery", String(describing:error)) } }
      }
    }
    return result
  }

  func installSnapshot(_ bytes: Data, required: Set<UInt64>) throws {
    guard bytes.count <= maxFrameBytes, let f = try? frame(bytes),
      let body = f.kind(type:ewe_ui_FrameMessage.self), body.messagesCount == 1,
      let op = body.messages(at:0)?.operation, op.uiType == .place,
      op.ui(type:ewe_ui_Place.self)?.kind == .fullview else { throw ViewRecoveryError.invalidSnapshot }
    let probe = NodeStore(maxFrameBytes:maxFrameBytes)
    probe.behaviours = behaviours
    guard probe.receive(bytes), probe.reports.isEmpty else { throw ViewRecoveryError.invalidSnapshot }
    var positions: [UInt64:UInt64] = [:]
    for node in probe.records.values {
      guard let spec = node.live, spec.kind == "Stream", spec.transport != "Local",
        !(spec.transport == "Sse" && spec.shared == true) else { continue }
      if let earlier = positions[spec.channelID], earlier != spec.resumeFrom { throw ViewRecoveryError.invalidSnapshot }
      positions[spec.channelID] = spec.resumeFrom
    }
    guard required.isSubset(of:Set(positions.keys)) else { throw ViewRecoveryError.missingCursor }
    clearViewOrdering()
    for (stream, cursor) in positions { setCursor(stream:stream, sequence:cursor) }
    guard receive(bytes) else { throw ViewRecoveryError.invalidSnapshot }
  }
}
