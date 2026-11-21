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

  func testCancelSheetAndOpenItAgain() {
    let app = XCUIApplication()
    app.launch()
    let add = app.navigationBars["Todos"].buttons["Add"]
    XCTAssertTrue(add.waitForExistence(timeout: 20))
    add.tap()
    let input = app.textFields["Todo title"]
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    XCTAssertFalse(add.exists && add.isHittable)
    app.buttons["Cancel"].tap()
    XCTAssertTrue(input.waitForNonExistence(timeout: 5))
    add.tap()
    XCTAssertTrue(input.waitForExistence(timeout: 5))
  }
}
