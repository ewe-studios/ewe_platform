import EweWire
//! Observable records for the native screen.
import Foundation
import Observation

public typealias NodeKind = ewe_ui_NodeKind
public typealias PropId = ewe_ui_PropId
public typealias EventId = ewe_ui_EventId

public struct Report: Equatable, Sendable {
  public let code: String
  public let row: Int
  public var node: UInt32? = nil
  public var name: String? = nil
}
public enum Value: Equatable, Sendable {
  case text(String)
  case bool(Bool)
  case length(Float, String)
  case color(UInt32)
  case font(UInt32)
  case axis(String)
  case alignment(String)
  public var description: String {
    switch self {
    case .text(let text), .axis(let text), .alignment(let text): text
    case .bool(let flag): flag.description
    case .color(let id): "color:\(id)"
    case .font(let id): "font:\(id)"
    case .length(let number, let unit):
      unit == "auto"
        ? unit
        : number.formatted(.number.locale(Locale(identifier: "en_US_POSIX")).grouping(.never))
          + unit
    }
  }
}
public struct LiveSpec: Equatable, Sendable {
  public let kind: String
  public let channelID: UInt64
  public let transport: String
  public let endpoint: String
  public let resumeFrom: UInt64
}
@MainActor @Observable public final class NodeRecord: Identifiable {
  public nonisolated let id: UInt32
  public var kind: NodeKind
  public var text: String?
  public var props: [PropId: Value] = [:]
  public var actions: [EventId: String] = [:]
  public var state = Data()
  public var children: [UInt32] = []
  public var parent: UInt32?
  public var behaviours: Set<String> = []
  public var live: LiveSpec?
  public var webViewURL: String?
  public var webViewParent: UInt32?
  init(_ id: UInt32, _ kind: NodeKind) {
    self.id = id
    self.kind = kind
  }
}
