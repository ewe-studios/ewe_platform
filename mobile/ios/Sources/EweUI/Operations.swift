import EweWire
import FlatBuffers
import Foundation

private protocol WireValue {
  var valueType: ewe_ui_PropValue { get }
  func value<T: FlatbuffersInitializable>(type: T.Type) -> T?
}
extension ewe_ui_SetProp: WireValue {}
extension ewe_ui_NativeArg: WireValue {}

private func value(_ op: any WireValue) -> Value? {
  switch op.valueType {
  case .text: return op.value(type: ewe_ui_TextValue.self)?.v.map(Value.text)
  case .bool: return op.value(type: ewe_ui_BoolValue.self).map { .bool($0.v) }
  case .colortoken: return op.value(type: ewe_ui_ColorToken.self).map { .color($0.id) }
  case .fonttoken: return op.value(type: ewe_ui_FontToken.self).map { .font($0.id) }
  case .axis:
    return op.value(type: ewe_ui_AxisValue.self).map { .axis(String(describing: $0.v).capitalized) }
  case .alignment:
    return op.value(type: ewe_ui_AlignmentValue.self).map {
      .alignment(String(describing: $0.v).capitalized)
    }
  case .length:
    guard let v = op.value(type: ewe_ui_LengthValue.self)?.v, v.value.isFinite else { return nil }
    return .length(v.value, v.unit == .auto ? "auto" : v.unit == .percent ? "%" : "pt")
  default: return nil
  }
}
@MainActor extension NodeStore {
  func operation(_ op: ewe_ui_Operation, _ row: Int, _ mount: UInt32) {
    if let batch = op.columnar { columnar(batch, row) }
    if op.uiType != .none_ { shared(op, row, mount) }
    if op.webType != .none_ { report("UnsupportedPlatformOp", row) }
    if op.mobileType != .none_ { mobile(op, row) }
    if op.uiType == .none_, op.webType == .none_, op.mobileType == .none_, op.columnar == nil {
      report("UnknownOp", row)
    }
  }
  private func shared(_ op: ewe_ui_Operation, _ row: Int, _ mount: UInt32) {
    switch op.uiType {
    case .createnode:
      if let v = op.ui(type: ewe_ui_CreateNode.self) { _ = make(v.nodeId, v.kind, row) }
    case .settext: if let v = op.ui(type: ewe_ui_SetText.self) { get(v.nodeId, row)?.text = v.text }
    case .setprop:
      if let v = op.ui(type: ewe_ui_SetProp.self), let n = get(v.nodeId, row) {
        if v.prop != .unknown, v.prop != .presentation, let prop = value(v) {
          setProperty(n, v.prop, prop, row)
        } else {
          report("UnknownProp", row, n.id)
        }
      }
    case .appendchild:
      if let v = op.ui(type: ewe_ui_AppendChild.self) { insert(v.childId, v.parentId, row) }
    case .insertbefore:
      if let v = op.ui(type: ewe_ui_InsertBefore.self) {
        insert(v.childId, v.parentId, before: v.beforeId, row)
      }
    case .remove: if let v = op.ui(type: ewe_ui_Remove.self) { remove(v.nodeId, row) }
    case .bindaction:
      if let v = op.ui(type: ewe_ui_BindAction.self) {
        get(v.nodeId, row)?.actions[v.event] = v.action
      }
    case .unbindaction:
      if let v = op.ui(type: ewe_ui_UnbindAction.self) {
        get(v.nodeId, row)?.actions[v.event] = nil
      }
    case .attachstate:
      if let v = op.ui(type: ewe_ui_AttachState.self) {
        get(v.rootNode, row)?.state = Data(v.state)
      }
    case .replacesubtree:
      if let v = op.ui(type: ewe_ui_ReplaceSubtree.self), let n = get(v.nodeId, row) {
        for child in n.children { remove(child, row) }
        for i in 0..<v.operationsCount {
          if let child = v.operations(at: i) { operation(child, Int(i), mount) }
        }
      }
    case .place: if let v = op.ui(type: ewe_ui_Place.self) { place(v, row, mount) }
    case .livemount:
      if let v = op.ui(type: ewe_ui_LiveMount.self), let n = make(v.nodeId, .stack, row) {
        n.live = LiveSpec(
          kind: String(describing: v.kind).capitalized, channelID: v.channelId,
          transport: String(describing: v.transport).capitalized, endpoint: v.endpoint ?? "",
          resumeFrom: v.resumeFrom,
          shared: v.sharing == .transportdefault ? nil : v.sharing == .shared, keepAlive: v.keepAlive)
      }
    case .uibehaviour:
      if let v = op.ui(type: ewe_ui_UiBehaviour.self), let n = get(v.nodeId, row) {
        let name = v.behaviour == .unknown ? String(v.rawBehaviour) : symbol(v.behaviour)
        if behaviours.has(name) {
          n.behaviours.insert(name)
        } else {
          report("UnknownBehaviour", row, n.id, name)
        }
      }
    default: report("UnknownOp", row)
    }
  }
  private func place(_ p: ewe_ui_Place, _ row: Int, _ mount: UInt32) {
    if let selector = p.target, selector.kind == .view, let name = selector.text,
      let destination = viewStore(named: name), destination !== self, let root = destination.viewRoots[name] {
      destination.place(p, row, root)
      destination.liveHost?.reconcile(destination)
      return
    }
    if p.kind == .fullview {
      guard mount != 0 else { report("AmbiguousView", row); return }
      guard let owner = get(mount, row), validateNames(p, scope: viewRoot(mount), removing: owner.children, row: row) else { return }
      for id in owner.children { remove(id, row) }
      for i in 0..<p.operationsCount {
        if let op = p.operations(at: i) { operation(op, Int(i), mount) }
      }
      insert(p.rootNode, mount, before: nil, row)
      return
    }
    guard let selector = p.target else { report("MissingTarget", row); return }
    let scope = selector.kind == .view ? selector.text.flatMap { viewRoots[$0] } : viewRoot(mount)
    guard let target = resolve(selector, scope: scope, row: row),
      validateNames(p, scope: scope, removing: p.kind == .swap ? [target.id] : [], row: row) else { return }
    if p.kind == .remove {
      remove(target.id, row)
      return
    }
    var parent = target
    var before: UInt32?
    if p.kind == .swap {
      guard let parentID = target.parent, let owner = records[parentID],
        let i = owner.children.firstIndex(of: target.id)
      else {
        report("MissingTarget", row, target.id)
        return
      }
      parent = owner
      if i + 1 < owner.children.count { before = owner.children[i + 1] }
      remove(target.id, row)
    } else if let child = p.child {
      guard let anchor = resolve(child, scope: scope, row: row), let i = parent.children.firstIndex(of: anchor.id) else {
        report("MissingTarget", row)
        return
      }
      if p.kind == .before {
        before = anchor.id
      } else if p.kind == .after, i + 1 < parent.children.count {
        before = parent.children[i + 1]
      }
    } else if p.kind == .before {
      before = parent.children.first
    }
    for i in 0..<p.operationsCount { if let op = p.operations(at: i) { operation(op, Int(i), mount) } }
    insert(p.rootNode, parent.id, before: before, row)
  }
  private func mobile(_ op: ewe_ui_Operation, _ row: Int) {
    switch op.mobileType {
    case .mountwebview:
      if let v = op.mobile(type: ewe_ui_MountWebView.self), let n = get(v.nodeId, row) {
        n.webViewURL = v.url
        n.webViewParent = v.parentId
      }
    case .mobilebehaviour:
      if let v = op.mobile(type: ewe_ui_MobileBehaviour.self), let n = get(v.nodeId, row) {
        let name = v.custom ?? String(v.rawBehaviour)
        if behaviours.has(name) {
          n.behaviours.insert(name)
        } else {
          report("UnknownBehaviour", row, n.id, name)
        }
      }
    case .nativecommand:
      if let v = op.mobile(type: ewe_ui_NativeCommand.self) {
        var args: [String: Value] = [:]
        for i in 0..<v.argsCount {
          if let a = v.args(at: i), let name = a.name, let typed = value(a) { args[name] = typed }
        }
        guard v.command != .unknown else {
          report("UnknownCommand", row)
          return
        }
        if let commands, !commands.run(v.command, args: args) {
          report("UnknownCommand", row)
          return
        }
        commandLog.append(["command": symbol(v.command), "args": args.mapValues(\.description)])
      }
    default: report("UnknownOp", row)
    }
  }
}
