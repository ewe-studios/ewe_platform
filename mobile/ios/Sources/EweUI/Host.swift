import EweWire
import Foundation
import SwiftUI

@MainActor public protocol CommandHost: AnyObject {
  func run(_ command: ewe_ui_NativeCommandId, args: [String: Value]) -> Bool
}
public struct EventInput: Sendable {
  public let nodeId: UInt32
  public let eventName: String
  public let value: String
  public let checked: Bool
  public let parentNode: UInt32
  public let state: Data
  public let json: Data?
  public init(
    nodeId: UInt32, eventName: String, value: String, checked: Bool, parentNode: UInt32, state: Data, json: Data? = nil
  ) {
    self.nodeId = nodeId
    self.eventName = eventName
    self.value = value
    self.checked = checked
    self.parentNode = parentNode
    self.state = state
    self.json = json
  }
}
@MainActor public protocol ActionCore: AnyObject {
  func trigger(reference: String, event: EventInput) async throws -> Data?
  func applyCapsFrame(_ frame: Data) throws
}
@MainActor public final class EventBridge {
  let store: NodeStore
  let core: any ActionCore
  public init(store: NodeStore, core: any ActionCore) {
    self.store = store
    self.core = core
    store.onCaps = { [weak store, weak core] frame in
      do { try core?.applyCapsFrame(frame) } catch {
        store?.error("ActionFailed", String(describing: error))
      }
    }
  }
  /// Find a named element in the view without exposing its renderer handle.
  public func containsElement(_ name: String, inView view: String) -> Bool {
    store.viewStore(named: view)?.containsElement(named: name) == true
  }

  /// Send typed input to a host action in a named view.
  public func triggerStructured<E: Encodable>(_ reference: String, inView view: String, value: E) async throws {
    guard let destination = store.viewStore(named: view) else { throw ViewTargetError.missingView }
    let bytes = try JSONEncoder().encode(value)
    let input = EventInput(nodeId: 0, eventName: "host", value: "", checked: false, parentNode: 0, state: Data(), json: bytes)
    if let reply = try await core.trigger(reference: reference, event: input) {
      try Task.checkCancellation()
      guard destination.receive(reply) else { throw CocoaError(.fileReadCorruptFile) }
    }
  }

  public func reportError(_ code: String, _ message: String) {
    store.error(code, message)
  }

  public func send(_ event: EventId, node: UInt32, value: String = "", checked: Bool = false) {
    submit(event, node: node, value: value, checked: checked, json: nil)
  }

  /// Submit a value defined by the form author with the state of the triggering control.
  public func sendStructured<E: Encodable>(_ event: EventId, node: UInt32, value: E) throws {
    let data = try JSONEncoder().encode(value)
    submit(event, node: node, value: "", checked: false, json: data)
  }

  private func submit(_ event: EventId, node: UInt32, value: String, checked: Bool, json: Data?) {
    guard let n = store.record(node), let reference = n.actions[event] else { return }
    let input = EventInput(
      nodeId: node, eventName: String(describing: event), value: value, checked: checked,
      parentNode: n.parent ?? 0, state: n.state, json: json)
    Task {
      do {
        if let reply = try await core.trigger(reference: reference, event: input) {
          store.receive(reply)
        }
      } catch { store.error("ActionFailed", String(describing: error)) }
    }
  }
}

@MainActor public final class BehaviourRegistry {
  public typealias Modifier = (AnyView, NodeRecord) -> AnyView
  var bindings: [String: Modifier] = [:]
  public let motion: MotionSettings
  public init(motion: MotionSettings = MotionSettings()) {
    self.motion = motion
    register("SCROLL_LOCK") { view, _ in AnyView(view.modifier(ScrollLockBinding())) }
    register("DIALOG") { view, node in
      AnyView(DialogBinding(content: view, allowsDismiss: node.behaviours.contains("DISMISS")))
    }
    register("DISMISS") { view, _ in AnyView(view.modifier(DismissBinding(motion: motion))) }
    register("TRANSITION") { view, node in
      AnyView(view.modifier(TransitionBinding(node: node, motion: motion)))
    }
  }
  public func register(_ name: String, _ apply: @escaping Modifier) { bindings[name] = apply }
  func has(_ name: String) -> Bool { bindings[name] != nil }
  func apply(_ view: AnyView, to node: NodeRecord) -> AnyView {
    node.behaviours.sorted { lhs, rhs in lhs == "DIALOG" ? false : rhs == "DIALOG" || lhs < rhs }
      .reduce(view) { current, name in
        bindings[name]?(current, node) ?? current
      }
  }
}

@MainActor public protocol Channel: AnyObject {
  func close()
  func send(_ bytes: Data) async throws
}
extension Channel {
  public func send(_ bytes: Data) async throws { throw ConnectionError.unsupported }
}
@MainActor public protocol ChannelFactory {
  func open(
    endpoint: URL, transport: String, streamId: UInt64, resumeFrom: UInt64,
    onFrame: @escaping @MainActor (Data) -> Void, onError: @escaping @MainActor (String) -> Void
  ) -> any Channel
}
@MainActor public final class LiveMountHost {
  private let factory: any ChannelFactory
  private var open: [UInt32: any Channel] = [:]
  public let connections: ConnectionManager?
  private let ownsConnections: Bool
  private var closeHandlers: [() -> Void] = []
  func onClose(_ handler: @escaping () -> Void) { closeHandlers.append(handler) }
  public init(factory: any ChannelFactory, connections: ConnectionManager? = nil) {
    self.factory = factory
    self.connections = connections ?? (factory as? URLSessionChannelFactory)?.connectionManager()
    self.ownsConnections = connections == nil
  }
  func forView() -> LiveMountHost { LiveMountHost(factory: factory, connections: connections) }

  func reconcile(_ store: NodeStore) {
    var attached = Set<UInt32>()
    var nodes: [UInt32] = store.viewRoots.isEmpty ? [1] : store.viewRoots.values.filter { store.childStores[$0] == nil }
    while let id = nodes.popLast(), let n = store.record(id) {
      nodes.append(contentsOf: n.children)
      guard let live = n.live, live.transport != "Local" else { continue }
      attached.insert(id)
      if open[id] == nil { mount(id, live, store: store) }
    }
    for id in Array(open.keys) where !attached.contains(id) { unmount(id) }
  }
  func mount(_ id: UInt32, _ spec: LiveSpec, store: NodeStore) {
    guard open[id] == nil, spec.transport != "Local", let url = URL(string: spec.endpoint) else {
      return
    }
    if let connections, connections.supports(spec.transport),
      spec.keepAlive || (spec.shared ?? store.subscriptionReplay) {
      do {
        open[id] = try LiveSubscription(manager: connections, spec: spec, endpoint: url, mount: id, store: store)
      } catch { store.error("Transport", String(describing: error)) }
      return
    }
    let generation = store.recovery?.generation
    open[id] = factory.open(
      endpoint: url, transport: spec.transport, streamId: spec.channelID,
      resumeFrom: spec.resumeFrom, onFrame: { [weak store] data in
        guard let store, generation.map({ store.recovery?.accepts($0) == true }) != false else { return }
        store.receive(data, at:id)
      },
      onError: { [weak store] error in store?.error("Transport", error) })
  }
  func reconnect(stream: UInt64, cursor: UInt64, store: NodeStore) {
    for id in Array(open.keys) {
      guard let spec = store.record(id)?.live, spec.channelID == stream else { continue }
      if store.subscriptionReplay, let subscription = open[id] as? LiveSubscription { subscription.resume(cursor); continue }
      unmount(id)
      let resumed = LiveSpec(
        kind: spec.kind, channelID: stream, transport: spec.transport, endpoint: spec.endpoint,
        resumeFrom: cursor, shared: spec.shared, keepAlive: spec.keepAlive)
      mount(id, resumed, store: store)
    }
  }
  func unmount(_ id: UInt32) { open.removeValue(forKey: id)?.close() }
  public func close() {
    for id in Array(open.keys) { unmount(id) }
    closeHandlers.forEach { $0() }
    closeHandlers.removeAll()
    if ownsConnections { connections?.close() }
  }
}
