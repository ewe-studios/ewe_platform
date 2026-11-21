import XCTest

final class TodoTests: XCTestCase {
  func testCreateFilterCompleteReopenAndDelete() {
    let app = XCUIApplication()
    app.launch()
    XCTAssertTrue(app.navigationBars["Todos"].waitForExistence(timeout: 20))
    XCTAssertTrue(app.staticTexts["0 open todos"].exists)

    app.navigationBars["Todos"].buttons["Add"].tap()
    let input = app.textFields["Todo title"]
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    input.tap()
    input.typeText("Device-only task")
    let submit = app.buttons.matching(identifier: "Add").allElementsBoundByIndex.first { $0.isHittable }
    XCTAssertNotNil(submit)
    submit?.tap()
    let task = app.staticTexts["Device-only task"]
    XCTAssertTrue(task.waitForExistence(timeout: 10))
    XCTAssertTrue(app.staticTexts["1 open todo"].waitForExistence(timeout: 5))

    app.buttons["Complete"].tap()
    XCTAssertTrue(app.buttons["Reopen"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts["0 open todos"].waitForExistence(timeout: 5))
    app.segmentedControls.buttons["Open"].tap()
    XCTAssertTrue(task.waitForNonExistence(timeout: 5))
    app.segmentedControls.buttons["Done"].tap()
    XCTAssertTrue(task.waitForExistence(timeout: 5))
    app.buttons["Reopen"].tap()
    app.segmentedControls.buttons["All"].tap()
    XCTAssertTrue(task.waitForExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts["1 open todo"].waitForExistence(timeout: 5))

    let screen = XCTAttachment(screenshot: app.screenshot())
    screen.name = "Todo On Device"
    screen.lifetime = .keepAlways
    add(screen)

    task.swipeLeft()
    app.buttons["Delete"].tap()
    let alert = app.alerts.firstMatch
    XCTAssertTrue(alert.waitForExistence(timeout: 5))
    alert.buttons["Confirm"].tap()
    XCTAssertTrue(task.waitForNonExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts["0 open todos"].waitForExistence(timeout: 5))
  }

  func testThemeReviewScreens() throws {
    try XCTSkipUnless(ProcessInfo.processInfo.environment["EWE_REVIEW_THEMES"] == "1")
    let originalAppearance = XCUIDevice.shared.appearance
    defer { XCUIDevice.shared.appearance = originalAppearance }
    for mode in ["light", "dark", "branded"] {
      let app = XCUIApplication()
      app.launchEnvironment["EWE_TODO_THEME"] = mode
      app.launchEnvironment["EWE_TODO_APPEARANCE"] = mode == "dark" ? "dark" : "light"
      XCUIDevice.shared.appearance = mode == "dark" ? .dark : .light
      if mode == "branded" {
        app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityL"]
      }
      app.launch()
      let addButton = app.navigationBars["Todos"].buttons["Add"]
      XCTAssertTrue(addButton.waitForExistence(timeout: 15))
      addButton.tap()
      let input = app.textFields["Todo title"]
      XCTAssertTrue(input.waitForExistence(timeout: 5))
      input.tap()
      let title = "Buy milk and collect the groceries for dinner"
      input.typeText(title)
      input.tap()
      let keyboard = app.keyboards.firstMatch
      XCTAssertTrue(keyboard.waitForExistence(timeout: 5), "The entry review needs the software keyboard")
      XCTAssertTrue(keyboard.keys.firstMatch.isHittable, "A software key must accept a tap")
      XCTAssertGreaterThan(app.frame.intersection(keyboard.frame).height, 100)
      let cancelButton = app.buttons["Cancel"]
      let submitButton = try XCTUnwrap(app.buttons.matching(identifier: "Add").allElementsBoundByIndex.first { $0.isHittable })
      XCTAssertLessThanOrEqual(cancelButton.frame.maxX, submitButton.frame.minX)
      XCTAssertLessThan(cancelButton.frame.minY, submitButton.frame.maxY)
      XCTAssertLessThan(submitButton.frame.minY, cancelButton.frame.maxY)
      XCTAssertLessThanOrEqual(max(cancelButton.frame.maxY, submitButton.frame.maxY), keyboard.frame.minY + 1)
      capture(app, "ios-\(mode)-entry")
      input.typeText("\n")
      let task = app.staticTexts[title]
      XCTAssertTrue(task.waitForExistence(timeout: 5))
      app.buttons["Complete"].tap()
      XCTAssertTrue(app.buttons["Reopen"].waitForExistence(timeout: 5))
      capture(app, "ios-\(mode)-list")
      task.swipeLeft()
      app.buttons["Delete"].tap()
      XCTAssertTrue(app.alerts.firstMatch.waitForExistence(timeout: 5))
      capture(app, "ios-\(mode)-confirmation")
      app.terminate()
    }
  }

  private func capture(_ app: XCUIApplication, _ name: String) {
    let image = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
    image.name = name
    image.lifetime = .keepAlways
    add(image)
  }

  func testCancelSheetAndOpenItAgain() {
    let app = XCUIApplication()
    app.launch()
    let add = app.navigationBars["Todos"].buttons["Add"]
    XCTAssertTrue(add.waitForExistence(timeout: 20))
    add.tap()
    let input = app.textFields["Todo title"]
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    XCTAssertFalse(add.exists && add.isHittable)
    input.tap()
    input.typeText("Discard this title")
    app.buttons["Cancel"].tap()
    XCTAssertFalse(app.staticTexts["Discard this title"].exists)
    XCTAssertTrue(input.waitForNonExistence(timeout: 5))
    add.tap()
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    XCTAssertNotEqual(input.value as? String, "Discard this title")
  }
}
