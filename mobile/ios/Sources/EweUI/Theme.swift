import Foundation
import SwiftUI

/// Colors, fonts, and named utilities for one screen.
public struct EweTheme {
  public var colors: [UInt32: Color]
  public var fonts: [UInt32: Font]
  private var utilities: [String: [String: [String]]] = [:]

  public init(colors: [UInt32: Color] = [:], fonts: [UInt32: Font] = [:]) {
    self.colors = colors
    self.fonts = fonts
  }

  /// Load the data from GeneratedTheme.native_manifest in the Rust application.
  public init(manifest: Data, colors: [UInt32: Color] = [:], fonts: [UInt32: Font] = [:]) throws {
    self.init(colors: colors, fonts: fonts)
    utilities = try JSONDecoder().decode([String: [String: [String]]].self, from: manifest)
  }

  func values(_ classes: String, dark: Bool) -> [String: String] {
    var values: [String: String] = [:]
    let words = classes.split(whereSeparator: { $0.isWhitespace })
    for specific in [false, true] {
      for word in words where word.hasPrefix("ios-") == specific {
        let name = specific ? String(word.dropFirst(4)) : String(word)
        for (key, pair) in utilities[name] ?? [:] where pair.count == 2 {
          let value = pair[dark ? 1 : 0]
          if (key == "color" || key == "background") ? Self.color(value) != nil : Self.length(value) != nil {
            values[key] = value
          }
        }
      }
    }
    return values
  }

  static func length(_ value: String) -> CGFloat? {
    let number = value.hasSuffix("px") || value.hasSuffix("pt") ? String(value.dropLast(2)) : value
    guard let points = Double(number), points.isFinite, points >= 0 else { return nil }
    return CGFloat(points)
  }

  static func color(_ string: String) -> Color? {
    guard string.hasPrefix("#") else { return nil }
    var hex = String(string.dropFirst())
    if hex.count == 3 || hex.count == 4 { hex = hex.map { String(repeating: String($0), count: 2) }.joined() }
    if hex.count == 6 { hex += "ff" }
    guard hex.count == 8, let rgba = UInt32(hex, radix: 16) else { return nil }
    func channel(_ shift: Int) -> Double { Double((rgba >> shift) & 255) / 255 }
    return Color(.sRGB, red: channel(24), green: channel(16), blue: channel(8), opacity: channel(0))
  }
}

struct EweNodeStyle: ViewModifier {
  let theme: EweTheme
  let node: NodeRecord
  @Environment(\.colorScheme) private var scheme
  @ScaledMetric private var fontScale: CGFloat = 1

  func body(content: Content) -> some View {
    let values = theme.values(node.props[.utilityClasses]?.description ?? "", dark: scheme == .dark)
    var padding = values["padding"].flatMap(EweTheme.length) ?? 0
    if case .length(let points, "pt") = node.props[.padding] { padding = CGFloat(points) }
    var color = values["color"].flatMap(EweTheme.color)
    if case .color(let token) = node.props[.color] { color = theme.colors[token] }
    var background = values["background"].flatMap(EweTheme.color)
    if case .color(let token) = node.props[.background] { background = theme.colors[token] }
    var font = values["font-size"].flatMap(EweTheme.length).map { Font.system(size: $0 * fontScale) }
    if case .font(let token) = node.props[.font] { font = theme.fonts[token] }
    return content.padding(padding)
      .foregroundColor(color)
      .background(background ?? Color.clear)
      .transformEnvironment(\.font) { if let font { $0 = font } }
  }
}
