import Foundation
import XCTest
@testable import EweUI

final class ThemeTests: XCTestCase {
  func testPlatformClassesOverrideSharedValuesInBothAppearances() throws {
    let json = """
      {"text-primary":{"color":["#112233","#aabbcc"]},"p-md":{"padding":["16px","16px"]},
       "p-lg":{"padding":["24px","24px"]},"p-relative":{"padding":["20%","20%"]}}
      """
    let theme = try EweTheme(manifest: Data(json.utf8))
    let classes = "ios-p-lg web-only p-md ios-p-relative text-primary android-p-sm"
    XCTAssertEqual(theme.values(classes, dark: false), ["color":"#112233", "padding":"24px"])
    XCTAssertEqual(theme.values(classes, dark: true), ["color":"#aabbcc", "padding":"24px"])
  }
}
