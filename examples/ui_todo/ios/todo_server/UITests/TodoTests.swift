import XCTest
import EweCore
import EweTodoCore
import EweUI

final class TodoTests: XCTestCase {
  func testOrdinaryDataService() async throws {
    let base = try XCTUnwrap(ProcessInfo.processInfo.environment["EWE_DATA_URL"])
    let client = NativeHttpClient(client: URLSessionRpcClient())
    let todo = try await getTodo(client: client, baseUrl: base, id: 42)
    XCTAssertEqual(todo.id, 42)
    XCTAssertEqual(todo.title, "Buy milk")
    do {
      _ = try await getTodo(client: client, baseUrl: base, id: 999)
      XCTFail("Expected the service to return NotFound")
    } catch TodoServiceError.NotFound { }
  }

  func testAddTaskToFreshServerStore() {
    let app = XCUIApplication()
    app.launchEnvironment["EWE_TODO_URL"] =
      ProcessInfo.processInfo.environment["EWE_TODO_URL"] ?? "http://127.0.0.1:8080"
    app.launchEnvironment["EWE_TODO_RPC"] = ProcessInfo.processInfo.environment["EWE_TODO_RPC"]
    app.launch()
    let add = app.navigationBars["Todos"].buttons["Add"]
    XCTAssertTrue(add.waitForExistence(timeout: 20))
    XCTAssertFalse(app.staticTexts["Device-only task"].exists)
    add.tap()
    let input = app.textFields["Todo title"]
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    input.tap()
    input.typeText("Server integration task\n")
    XCTAssertTrue(app.staticTexts["Server integration task"].waitForExistence(timeout: 10))
    XCTAssertTrue(app.staticTexts["1 open todo"].waitForExistence(timeout: 5))
    app.buttons["Complete"].tap()
    XCTAssertTrue(app.buttons["Reopen"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts["0 open todos"].waitForExistence(timeout: 5))
    app.buttons["Reopen"].tap()
    XCTAssertTrue(app.staticTexts["1 open todo"].waitForExistence(timeout: 5))
  }
}
