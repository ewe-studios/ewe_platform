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
    XCTAssertFalse(app.staticTexts["Pending"].exists)

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

  func testSearchAndEntrySheet() throws {
    continueAfterFailure = false
    try verifySearchAndEntrySheet(appearance: "light")
    try verifySearchAndEntrySheet(appearance: "dark")
  }

  private func verifySearchAndEntrySheet(appearance: String) throws {
    let app = XCUIApplication()
    app.launchEnvironment["EWE_TODO_APPEARANCE"] = appearance
    if appearance == "dark" {
      app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityL"]
    }
    app.launch()
    defer { app.terminate() }
    let search = app.searchFields["Search todos"]
    XCTAssertTrue(search.waitForExistence(timeout: 20))
    let add = app.navigationBars["Todos"].buttons["Add"]
    add.tap()
    let input = app.textFields["Todo title"]
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    capture(app, "ios-\(appearance)-entry-attached")
    input.tap()
    let keyboard = app.keyboards.firstMatch
    XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
    let cancel = app.buttons["Cancel"]
    let submit = try XCTUnwrap(app.buttons.matching(identifier: "Add").allElementsBoundByIndex.first { $0.isHittable })
    XCTAssertLessThanOrEqual(max(submit.frame.maxY, cancel.frame.maxY), keyboard.frame.minY + 1)
    typeWithKeyboard("Review search", keyboard)
    capture(app, "ios-\(appearance)-entry-keyboard")
    submit.tap()
    let row = app.staticTexts["Review search"]
    XCTAssertTrue(row.waitForExistence(timeout: 10))
    capture(app, "ios-\(appearance)-search-list")
    search.tap()
    search.typeText("missing")
    XCTAssertTrue(row.waitForNonExistence(timeout: 5))
    let clear = search.buttons.firstMatch
    XCTAssertTrue(clear.waitForExistence(timeout: 5))
    clear.tap()
    XCTAssertTrue(row.waitForExistence(timeout: 5))
    search.typeText("Review\n")
    XCTAssertTrue(keyboard.waitForNonExistence(timeout: 5))
    XCTAssertTrue(row.exists)
  }

  func testThemeReviewScreens() throws {
    try XCTSkipUnless(ProcessInfo.processInfo.environment["EWE_REVIEW_THEMES"] == "1")
    continueAfterFailure = false
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
      let keyboard = app.keyboards.firstMatch
      XCTAssertTrue(keyboard.waitForExistence(timeout: 5), "The entry review needs the software keyboard")
      let visibleKeyboard = NSPredicate { _, _ in
        app.frame.intersection(keyboard.frame).height > 100
      }
      let visible = expectation(for: visibleKeyboard, evaluatedWith: keyboard)
      wait(for: [visible], timeout: 5)
      XCTAssertTrue(keyboard.keys.allElementsBoundByIndex.contains { $0.isHittable })
      typeWithKeyboard(title, keyboard)
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

  /// Tap the software keys to keep the keyboard visible for the capture.
  private func typeWithKeyboard(_ text: String, _ keyboard: XCUIElement) {
    for character in text {
      let label = character == " " ? "space" : String(character)
      let key = keyboard.keys.matching(NSPredicate(format: "label ==[c] %@", label)).firstMatch
      XCTAssertTrue(key.exists, "Missing software key \(label)")
      key.tap()
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
