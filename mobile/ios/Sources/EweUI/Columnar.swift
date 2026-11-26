import EweWire
import Foundation

@MainActor extension NodeStore {
  func columnar(_ batch: ewe_ui_ColumnarBatch, _ base: Int) {
    let n = batch.opCodesCount
    let length = batch.blobBytesCount
    guard n < Int32.max, batch.nodeIdsCount == n, batch.argACount == n, batch.argBCount == n,
      batch.blobAOffCount == n + 1, batch.blobBOffCount == n + 1,
      batch.blobAOff(at: 0) == 0, batch.blobAOff(at: n) == UInt32(length)
    else {
      report("BadFrame", base)
      return
    }
    let second = batch.blobBOff(at: 0)
    guard second == 0 || second == UInt32(length),
      (0...n).allSatisfy({ batch.blobBOff(at: $0) == second }),
      (0..<n).allSatisfy({ batch.blobAOff(at: $0) <= batch.blobAOff(at: $0 + 1) })
    else {
      report("BadFrame", base)
      return
    }
    for i in 0..<n {
      let id = batch.nodeIds(at: i)
      let arg = batch.argA(at: i)
      let bits = batch.argB(at: i)
      let row = base + Int(i)
      let indices = Int32(batch.blobAOff(at: i))..<Int32(batch.blobAOff(at: i + 1))
      let bytes = indices.lazy.map { batch.blobBytes(at: $0) }
      func string() -> String? { String(bytes: bytes, encoding: .utf8) }
      let code = batch.opCodes(at: i)
      switch ewe_ui_UiOperation(rawValue: code) {
      case .createnode:
        _ = make(id, UInt16(exactly: arg).flatMap(NodeKind.init(rawValue:)) ?? .unknown, row)
      case .settext:
        if let string = string() { get(id, row)?.text = string } else { report("BadFrame", row) }
      case .setprop:
        guard let node = get(id, row), let prop = PropId(rawValue: UInt16(truncatingIfNeeded: arg)),
          prop != .unknown, prop != .presentation
        else {
          report("UnknownProp", row, id)
          continue
        }
        let value: Value?
        switch prop {
        case .padding, .spacing, .height, .width:
          let unit = arg >> 16
          let f = Float(bitPattern: bits)
          value = unit < 3 && f.isFinite ? .length(f, ["pt", "%", "auto"][Int(unit)]) : nil
        case .color, .background: value = .color(bits)
        case .font: value = .font(bits)
        case .align:
          value = UInt8(exactly: bits).flatMap(ewe_ui_Alignment.init(rawValue:)).map {
            .alignment(String(describing: $0).capitalized)
          }
        case .axis:
          value = UInt8(exactly: bits).flatMap(ewe_ui_Axis.init(rawValue:)).map {
            .axis(String(describing: $0).capitalized)
          }
        case .enabled, .checked, .hidden: value = bits <= 1 ? .bool(bits == 1) : nil
        default: value = string().map(Value.text)
        }
        if let value { setProperty(node, prop, value, row) } else { report("BadFrame", row) }
      case .appendchild: insert(arg, id, row)
      case .insertbefore: insert(arg, id, before: bits, row)
      case .remove: remove(id, row)
      case .attachstate: get(id, row)?.state = Data(bytes)
      case .bindaction, .unbindaction:
        guard let event = UInt16(exactly: arg).flatMap(EventId.init(rawValue:)) else {
          report("BadFrame", row)
          continue
        }
        get(id, row)?.actions[event] =
          code == ewe_ui_UiOperation.bindaction.rawValue ? string() : nil
      default: report("UnknownOp", row)
      }
    }
  }
}
