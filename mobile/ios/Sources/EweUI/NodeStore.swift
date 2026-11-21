import EweWire
import FlatBuffers
import Foundation
import Observation

@MainActor @Observable public final class NodeStore {
  @ObservationIgnored var records: [UInt32: NodeRecord] = [1: NodeRecord(1, .stack)]
  public internal(set) var reports: [Report] = []
  @ObservationIgnored var commandLog: [[String: Any]] = []
  @ObservationIgnored var dataLog: [[String: Any]] = []
  @ObservationIgnored var errorLog: [[String: Any]] = []
  @ObservationIgnored var capsLog: [[String: Any]] = []
  @ObservationIgnored private var cursors: [UInt64: UInt64] = [:]
  @ObservationIgnored private var pending: [UInt64: [UInt64: (data: Data, mount: UInt32)]] = [:]
  @ObservationIgnored private var gapTasks: [UInt64: Task<Void, Never>] = [:]
  public let maxFrameBytes: Int
  public let maxPendingFrames: Int
  let gapTimeout: Duration
  let sleep: @Sendable (Duration) async throws -> Void
  @ObservationIgnored public var onGap: ((UInt64, UInt64) -> Void)?
  @ObservationIgnored public var onError: ((String, String) -> Void)?
  @ObservationIgnored public var onData: ((UInt32, ewe_ui_Data) -> Void)?
  @ObservationIgnored public var onCaps: ((Data) -> Void)?
  @ObservationIgnored public var commands: (any CommandHost)?
  @ObservationIgnored public var liveHost: LiveMountHost?
  @ObservationIgnored public var behaviours = BehaviourRegistry()

  public init(
    gapTimeout: Duration = .milliseconds(1000), maxFrameBytes: Int = 4 * 1024 * 1024,
    maxPendingFrames: Int = 128, commands: (any CommandHost)? = nil, live: LiveMountHost? = nil,
    sleep: @escaping @Sendable (Duration) async throws -> Void = { try await Task.sleep(for: $0) }
  ) {
    self.sleep = sleep
    self.gapTimeout = gapTimeout
    self.maxFrameBytes = maxFrameBytes
    self.maxPendingFrames = maxPendingFrames
    self.commands = commands
    self.liveHost = live
  }
  public func record(_ id: UInt32) -> NodeRecord? { records[id] }
  public func setCursor(stream: UInt64, sequence: UInt64) {
    gapTasks.removeValue(forKey: stream)?.cancel()
    pending.removeValue(forKey: stream)
    cursors[stream] = sequence
  }
  func report(_ code: String, _ row: Int, _ node: UInt32? = nil, _ name: String? = nil) {
    reports.append(Report(code: code, row: row, node: node, name: name))
  }
  func get(_ id: UInt32, _ row: Int) -> NodeRecord? {
    if let node = records[id] { return node }
    report("UnknownNode", row, id)
    return nil
  }
  private func frame(_ data: Data) throws -> ewe_ui_Frame {
    var bytes = ByteBuffer(data: data)
    return try getCheckedRoot(byteBuffer: &bytes, fileId: "EWF1")
  }
  @discardableResult public func receive(_ data: Data, at mount: UInt32 = 1) -> Bool {
    guard data.count <= maxFrameBytes else {
      report("TooLarge", 0)
      return false
    }
    guard data.count >= 8, let frame = try? frame(data), frame.schemaVersion == 1,
      frame.kindType == .framemessage, let body = frame.kind(type: ewe_ui_FrameMessage.self)
    else {
      report("BadFrame", 0)
      return false
    }
    let stream = frame.streamId
    let seq = frame.sequenceId
    if body.ack != 0 || seq == 0 {
      apply(body, data, mount)
      return true
    }
    if let last = cursors[stream] {
      if seq <= last { return true }
      if seq > last + 1 {
        if pending[stream]?[seq] != nil { return true }
        guard (pending[stream]?.count ?? 0) < maxPendingFrames else {
          report("TooManyPending", 0)
          return false
        }
        pending[stream, default: [:]][seq] = (data, mount)
        if gapTasks[stream] == nil {
          gapTasks[stream] = Task { @MainActor [weak self, gapTimeout, sleep] in
            do {
              try await sleep(gapTimeout)
              try Task.checkCancellation()
            } catch { return }
            guard let self else { return }
            self.pending[stream] = nil
            self.gapTasks[stream] = nil
            let cursor = self.cursors[stream] ?? 0
            self.liveHost?.reconnect(stream: stream, cursor: cursor, store: self)
            self.onGap?(stream, cursor)
          }
        }
        return true
      }
    }
    apply(body, data, mount)
    cursors[stream] = seq
    while let last = cursors[stream], last < UInt64.max,
      let next = pending[stream]?.removeValue(forKey: last + 1),
      let ordered = try? self.frame(next.data),
      let message = ordered.kind(type: ewe_ui_FrameMessage.self)
    {
      apply(message, next.data, next.mount)
      cursors[stream] = last + 1
    }
    if pending[stream]?.isEmpty != false {
      gapTasks.removeValue(forKey: stream)?.cancel()
      pending[stream] = nil
    }
    return true
  }
  private func apply(_ frame: ewe_ui_FrameMessage, _ bytes: Data, _ mount: UInt32) {
    var acceptedCaps = false
    for index in 0..<frame.messagesCount {
      guard let msg = frame.messages(at: index) else { continue }
      if let op = msg.operation { operation(op, Int(index), mount) }
      if let data = msg.data {
        dataLog.append([
          "data": data.arrow != nil ? "ArrowIpc" : data.table != nil ? "Table" : "MessagePack",
          "target": data.targetNode,
        ])
        onData?(data.targetNode, data)
      }
      if let control = msg.control {
        if let caps = control.caps {
          capsLog.append([
            "stream_id": caps.streamId, "resume_from": caps.resumeFrom,
            "status": variant(caps.status),
          ])
          acceptedCaps = true
        }
        if let err = control.error {
          let code = variant(err.code)
          errorLog.append([
            "code": code, "message": err.message as Any? ?? NSNull(), "row": err.rowIndex,
          ])
          onError?(code, err.message ?? "")
        }
        if control.stateExpired != nil { error("StateExpired", "") }
        if control.updateRequired != nil { error("UpdateRequired", "") }
      }
    }
    liveHost?.reconcile(self)
    if acceptedCaps { onCaps?(bytes) }
  }
  func error(_ code: String, _ message: String) {
    errorLog.append(
      message.isEmpty && ["StateExpired", "UpdateRequired"].contains(code)
        ? ["code": code] : ["code": code, "message": message])
    onError?(code, message)
  }
  func make(_ id: UInt32, _ kind: NodeKind, _ row: Int) -> NodeRecord? {
    guard id > 2 else {
      report("ReservedNode", row, id)
      return nil
    }
    if records[id] != nil {
      report("DuplicateNode", row, id)
      remove(id, row)
    }
    if kind == .unknown { report("UnknownKind", row, id) }
    let node = NodeRecord(id, kind)
    records[id] = node
    return node
  }
  func detach(_ node: NodeRecord) {
    if let id = node.parent { records[id]?.children.removeAll { $0 == node.id } }
    node.parent = nil
  }
  func insert(_ childID: UInt32, _ parentID: UInt32, before: UInt32? = nil, _ row: Int) {
    guard let parent = get(parentID, row), let child = get(childID, row) else { return }
    if let before, !parent.children.contains(before) {
      report("UnknownNode", row, before)
      return
    }
    if before == childID { return }
    var ancestor: NodeRecord? = parent
    while let node = ancestor {
      if node.id == childID {
        report("InvalidParent", row, childID)
        return
      }
      ancestor = node.parent.flatMap { records[$0] }
    }
    guard childID != 1 else {
      report("ReservedNode", row, childID)
      return
    }
    detach(child)
    if let before, let index = parent.children.firstIndex(of: before) {
      parent.children.insert(childID, at: index)
    } else {
      parent.children.append(childID)
    }
    child.parent = parentID

  }
  func remove(_ id: UInt32, _ row: Int) {
    guard let node = get(id, row) else { return }
    guard id != 1 else {
      report("ReservedNode", row, id)
      return
    }
    var stack = [node]
    detach(node)
    while let next = stack.popLast() {
      stack.append(contentsOf: next.children.compactMap { records[$0] })
      liveHost?.unmount(next.id)
      records[next.id] = nil
    }
  }
  public func snapshotJSON() throws -> Data {
    let null = NSNull()
    let nodes: [[String: Any]] = records.values.sorted { $0.id < $1.id }.map { n in
      let live: Any =
        n.live.map {
          [
            "kind": $0.kind, "channel_id": $0.channelID, "transport": $0.transport,
            "endpoint": $0.endpoint, "resume_from": $0.resumeFrom,
          ] as [String: Any]
        } as Any? ?? null
      let web: Any =
        n.webViewURL.map { ["parent_id": n.webViewParent ?? 0, "url": $0] as [String: Any] } as Any?
        ?? null
      return [
        "id": n.id, "kind": symbol(n.kind), "text": n.text as Any? ?? null,
        "props": Dictionary(
          uniqueKeysWithValues: n.props.map { (symbol($0.key), $0.value.description) }),
        "children": n.children,
        "actions": Dictionary(uniqueKeysWithValues: n.actions.map { (symbol($0.key), $0.value) }),
        "state_len": n.state.count, "live": live, "web_view": web,
        "behaviours": n.behaviours.sorted(),
      ]
    }
    let reports: [[String: Any]] = reports.map { r in
      var entry: [String: Any] = ["code": r.code, "row": r.row]
      entry["node"] = r.node
      entry["name"] = r.name
      return entry
    }
    var snapshot: [String: Any] = [
      "nodes": nodes, "reports": reports, "commands": commandLog, "errors": errorLog,
      "data": dataLog,
    ]
    if !capsLog.isEmpty { snapshot["caps"] = capsLog }
    return try JSONSerialization.data(withJSONObject: snapshot, options: [.sortedKeys])
  }
}

func symbol<T>(_ value: T) -> String {
  String(describing: value).unicodeScalars.reduce(into: "") { text, c in
    if CharacterSet.uppercaseLetters.contains(c), !text.isEmpty { text += "_" }
    text += String(c).uppercased()
  }
}

func variant<T>(_ value: T) -> String {
  let text = String(describing: value)
  return text.prefix(1).uppercased() + text.dropFirst()
}
