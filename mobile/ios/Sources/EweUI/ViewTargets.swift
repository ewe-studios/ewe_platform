import EweWire
import Foundation

public enum ViewTargetError: Error { case invalidName, duplicateName, duplicateView, missingView }

@MainActor extension NodeStore {
  /// Register a named view. The store allocates its root.
  @discardableResult public func registerView(_ name: String) throws -> NodeStore {
    var root: UInt32 = 1
    if viewRoots.values.contains(root) {
      repeat {
        guard root < UInt32.max else { throw ViewTargetError.missingView }
        root += 1
      } while records[root] != nil || viewRoots.values.contains(root)
    }
    try registerView(name, root: root)
    return childStores[root] ?? self
  }

  /// Register a view with its own node identifiers and stream positions.
  public func registerView(_ name: String, root: UInt32) throws {
    guard Self.validName(name), root != 0 else { throw ViewTargetError.invalidName }
    guard viewRoots[name] == nil, !viewRoots.values.contains(root) else { throw ViewTargetError.duplicateView }
    if root != 1 {
      let child = NodeStore(gapTimeout: gapTimeout, maxFrameBytes: maxFrameBytes,
        maxPendingFrames: maxPendingFrames, commands: commands, live: liveHost?.forView(), sleep: sleep)
      child.parentStore = self
      child.viewRoots[name] = root
      child.subscriptionReplay = subscriptionReplay
      child.behaviours = behaviours
      child.onSnapshot = { [weak self] id, stream in self?.onSnapshot?(id,stream) }
      child.onData = { [weak self] id, data in self?.onData?(id,data) }
      child.onError = { [weak self] code, message in self?.onError?(code,message) }
      child.onCaps = { [weak self] bytes in self?.onCaps?(bytes) }
      var pending = [root]
      while let id = pending.popLast(), let node = records.removeValue(forKey: id) {
        child.records[id] = node
        child.elementNames[id] = elementNames.removeValue(forKey: id)
        child.elementTags[id] = elementTags.removeValue(forKey: id)
        pending.append(contentsOf: node.children)
      }
      if child.records[root] == nil { child.records[root] = NodeRecord(root, .stack) }
      childStores[root] = child
    }
    viewRoots[name] = root
  }

  /// Use this store to display a named view or inspect its local node identifiers.
  public func viewStore(named name: String) -> NodeStore? {
    let host = parentStore ?? self
    guard let root = host.viewRoots[name] else { return nil }
    return host.childStores[root] ?? host
  }

  /// Check whether this view currently contains an element with the given name.
  public func containsElement(named name: String) -> Bool {
    elementNames.values.contains(name)
  }

  /// Give an existing custom control its portable tag and element name.
  public func registerElement(_ name: String, tag: String, node id: UInt32) throws {
    guard Self.validName(name), Self.validName(tag), tag.lowercased() != "view" else { throw ViewTargetError.invalidName }
    guard let node = records[id] else { throw ViewTargetError.missingView }
    if let root = viewRoot(id), elementNames.contains(where: { $0.key != id && $0.value == name && inside($0.key,root) }) {
      throw ViewTargetError.duplicateName
    }
    node.props[.id] = .text(name)
    elementNames[id] = name
    elementTags[id] = tag
  }

  public func unregisterView(_ name: String) throws {
    guard let root = viewRoots.removeValue(forKey: name) else { throw ViewTargetError.missingView }
    if let child = childStores.removeValue(forKey: root) {
      child.recovery?.close()
      child.liveHost?.close()
      child.clearViewOrdering()
      child.remove(root, 0)
      return
    }
    recovery?.close()
    clearViewOrdering()
    if root == 1 {
      for child in records[root]?.children ?? [] { remove(child, 0) }
    } else { remove(root, 0) }
  }

  func setProperty(_ node: NodeRecord, _ prop: PropId, _ value: Value, _ row: Int) {
    if prop == .id {
      let name = value.description
      guard Self.validName(name) else { report("InvalidName", row, node.id); return }
      if let scope = viewRoot(node.id), elementNames.contains(where: { $0.key != node.id && $0.value == name && inside($0.key,scope) }) {
        report("DuplicateName", row, node.id); return
      }
      elementNames[node.id] = name
    }
    node.props[prop] = value
  }

  static func validName(_ name: String) -> Bool {
    let letters = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_")
    let rest = letters.union(CharacterSet(charactersIn: "-0123456789"))
    guard let first = name.unicodeScalars.first, letters.contains(first) else { return false }
    return name.unicodeScalars.dropFirst().allSatisfy(rest.contains)
  }
  func viewRoot(_ mount: UInt32) -> UInt32? {
    if mount == 0 { return viewRoots.values.count == 1 ? viewRoots.values.first : nil }
    var node = records[mount]
    while let current = node {
      if viewRoots.values.contains(current.id) || current.id == 1 { return current.id }
      node = current.parent.flatMap { records[$0] }
    }
    return records[mount]?.id
  }
  func inside(_ id: UInt32, _ root: UInt32) -> Bool {
    var node = records[id]
    while let current = node {
      if current.id == root { return true }
      if viewRoots.values.contains(current.id) { return false }
      node = current.parent.flatMap { records[$0] }
    }
    return false
  }
  func resolve(_ selector: ewe_ui_TargetSelector, scope: UInt32?, row: Int) -> NodeRecord? {
    func fail(_ code: String) -> NodeRecord? { report(code, row); return nil }
    switch selector.kind {
    case .nodeid: return records[selector.nodeId] ?? fail("MissingTarget")
    case .view: return selector.text.flatMap { viewRoots[$0] }.flatMap { records[$0] } ?? fail("MissingTarget")
    case .receiving: return scope.flatMap { records[$0] } ?? fail("AmbiguousView")
    case .id, .typedid:
      guard let scope else { return fail("AmbiguousView") }
      guard let text = selector.text else { return fail("InvalidName") }
      let parts = text.components(separatedBy: "#")
      guard let name = parts.last, Self.validName(name),
        selector.kind == .id ? parts.count == 1 : parts.count == 2 && Self.validName(parts[0]) else { return fail("InvalidName") }
      let ids = elementNames.filter { $0.value == name && inside($0.key, scope) }
      guard ids.count == 1, let id = ids.first?.key, let target = records[id] else {
        return fail(ids.isEmpty ? "MissingTarget" : "DuplicateName")
      }
      if selector.kind == .typedid {
        let tag = parts[0].lowercased().filter { $0 != "-" && $0 != "_" }
        let actual = (elementTags[id] ?? String(describing: target.kind)).lowercased().filter { $0 != "-" && $0 != "_" }
        if actual != (tag == "sheet" ? "bottomsheet" : tag) { return fail("WrongTargetKind") }
      }
      return target
    default: return fail("UnsupportedSelector")
    }
  }
  func validateNames(_ p: ewe_ui_Place, scope: UInt32?, removing: [UInt32], row: Int) -> Bool {
    var names = Set(elementNames.compactMap { id, name in
      scope.map { inside(id, $0) } == true && !removing.contains(where: { inside(id, $0) }) ? name : nil
    })
    func check(_ name: String?) -> Bool {
      guard let name, Self.validName(name) else { report("InvalidName", row); return false }
      guard names.insert(name).inserted else { report("DuplicateName", row); return false }
      return true
    }
    for index in 0..<p.operationsCount {
      guard let op = p.operations(at: index) else { report("BadFrame", row); return false }
      if op.uiType == .setprop, let prop = op.ui(type: ewe_ui_SetProp.self), prop.prop == .id,
        !check(prop.value(type: ewe_ui_TextValue.self)?.v) { return false }
      guard let batch = op.columnar else { continue }
      guard batch.argACount == batch.opCodesCount, batch.opCodesCount < Int32.max,
        batch.blobAOffCount == batch.opCodesCount + 1 else { report("BadFrame",row); return false }
      for i in 0..<batch.opCodesCount where batch.opCodes(at: i) == ewe_ui_UiOperation.setprop.rawValue {
        guard UInt16(truncatingIfNeeded: batch.argA(at:i)) == PropId.id.rawValue else { continue }
        let start = batch.blobAOff(at:i), end = batch.blobAOff(at:i+1)
        guard start <= end, end <= batch.blobBytesCount else { report("BadFrame",row); return false }
        let bytes = (Int32(start)..<Int32(end)).lazy.map { batch.blobBytes(at:$0) }
        if !check(String(bytes:bytes,encoding:.utf8)) { return false }
      }
    }
    return true
  }
}
