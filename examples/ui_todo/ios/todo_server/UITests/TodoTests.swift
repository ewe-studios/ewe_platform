import XCTest
import EweCore
import EweTodoCore
@testable import EweUI
import EweWire
import FlatBuffers

final class TodoTests: XCTestCase {
  @MainActor func testSubscriptionReplayUsesTheOpenDelivery() async throws {
    let address = try XCTUnwrap(ProcessInfo.processInfo.environment["EWE_TODO_URL"])
    let base = try XCTUnwrap(URL(string: address))
    let manager = URLSessionChannelFactory(baseURL: base).connectionManager()
    defer { manager.close() }
    for transport in ["sse", "http", "websocket"] {
      let socket = transport == "websocket"
      let url = try XCTUnwrap(URL(string: socket ? "/socket" : "/live", relativeTo: base))
      let id = try manager.nextTag()
      let greeting = expectation(description: "\(transport) handshake")
      let expired = expectation(description: "\(transport) targeted expiry")
      var session: UInt64?
      func context(_ bytes: Data) {
        guard session == nil, let caps = SubscriptionFrames.caps(bytes), caps.subscriptionResume else { return }
        session = caps.streamId
        greeting.fulfill()
      }
      let handle = try manager.acquire(transport, endpoint: url, shared: true,
        onContext: context, onError: { XCTFail($0) }, receive: { bytes in
          if SubscriptionFrames.caps(bytes) != nil { context(bytes); return }
          var buffer = ByteBuffer(data: bytes)
          guard let frame: ewe_ui_Frame = try? getCheckedRoot(byteBuffer: &buffer, fileId: "EWF1"),
            let body = frame.kind(type: ewe_ui_FrameMessage.self), body.subscriptionId == id,
            body.messages(at: 0)?.control?.replayExpired?.streamId == 1 else { return }
          expired.fulfill()
        })
      defer { handle.close() }
      if socket { try await handle.send(SubscriptionFrames.hello(id)) }
      await fulfillment(of: [greeting], timeout: 10)
      try await handle.send(SubscriptionFrames.resume(session: try XCTUnwrap(session), id: id, stream: 1, after: UInt64.max))
      await fulfillment(of: [expired], timeout: 10)
    }
  }

  func testCountPublishedToOpenMobileAndWebViews() throws {
    let env = ProcessInfo.processInfo.environment
    guard let before = env["EWE_SHARED_COUNT_BEFORE"], let after = env["EWE_SHARED_COUNT_AFTER"] else {
      throw XCTSkip("This check runs with a browser and another native peer")
    }
    let app = XCUIApplication()
    app.launchEnvironment["EWE_TODO_URL"] = try XCTUnwrap(env["EWE_TODO_URL"])
    app.launch()
    XCTAssertTrue(app.staticTexts[before].waitForExistence(timeout: 20))
    print("iPhone subscribed at \(before)")
    XCTAssertTrue(app.staticTexts[after].waitForExistence(timeout: 90))
  }

  func testOrdinaryDataService() async throws {
    let base = try XCTUnwrap(ProcessInfo.processInfo.environment["EWE_DATA_URL"])
    let client = NativeHttpClient(client: URLSessionRpcClient())
    let id = "00000000-0000-4000-8000-000000000042"
    let todo = try await getTodo(client: client, baseUrl: base, id: id)
    XCTAssertEqual(todo.id, id)
    XCTAssertEqual(todo.title, "Buy milk")
    do {
      _ = try await getTodo(client: client, baseUrl: base, id: "00000000-0000-4000-8000-000000000999")
      XCTFail("Expected the service to return NotFound")
    } catch TodoServiceError.NotFound { }
    do {
      _ = try await getTodo(client: client, baseUrl: base, id: "42")
      XCTFail("Expected the client to reject a numeric identifier")
    } catch TodoServiceError.Failed(let detail) {
      XCTAssertTrue(detail.contains("UUID"))
    }
  }

  @MainActor func testBrowserPeerUpdatesRowsWithoutReplacingFilterOrDraft() async throws {
    let env = ProcessInfo.processInfo.environment
    guard let testAddress = env["EWE_TODO_PEER_TEST_URL"] else {
      throw XCTSkip("This check needs the browser peer test driver")
    }
    continueAfterFailure = false
    let driver = try XCTUnwrap(URL(string: testAddress))
    let allPlatforms = env["EWE_TODO_PEER_ANDROID"] == "1"
    let title = env["EWE_TODO_PEER_TITLE"] ?? "Browser peer todo"
    let count = Int(env["EWE_TODO_PEER_OPEN_COUNT"] ?? "0") ?? 0
    let label = "\(count) open \(count == 1 ? "todo" : "todos")"
    let app = XCUIApplication()
    app.launchEnvironment["EWE_TODO_URL"] = try XCTUnwrap(env["EWE_TODO_URL"])
    app.launch()
    let add = app.navigationBars["Todos"].buttons["Add"]
    XCTAssertTrue(add.waitForExistence(timeout: 20))

    func peer(_ action: String) async throws {
      var request = URLRequest(url: driver.appending(path: action))
      request.timeoutInterval = 120
      let (_, response) = try await URLSession.shared.data(for: request)
      XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 204)
    }

    let filters = app.segmentedControls.firstMatch
    let all = filters.coordinate(withNormalizedOffset: CGVector(dx: 1.0 / 6.0, dy: 0.5))
    let done = filters.coordinate(withNormalizedOffset: CGVector(dx: 5.0 / 6.0, dy: 0.5))
    let doneButton = filters.buttons["Done"]
    XCTAssertTrue(doneButton.waitForExistence(timeout: 5))
    done.tap()
    XCTAssertTrue(doneButton.isSelected)

    add.tap()
    let draft = app.textFields["Todo title"]
    XCTAssertTrue(draft.waitForExistence(timeout: 5))
    draft.tap()
    draft.typeText("Keep this draft")
    try await peer("add")
    XCTAssertEqual(draft.value as? String, "Keep this draft")
    app.buttons["Cancel"].tap()

    let row = app.staticTexts[title]
    XCTAssertFalse(row.exists)
    XCTAssertTrue(doneButton.isSelected)

    func checkBrowserOpenRow() {
      all.tap()
      XCTAssertTrue(row.waitForExistence(timeout: 10))
      XCTAssertTrue(app.buttons["Complete"].exists)
      done.tap()
      XCTAssertTrue(row.waitForNonExistence(timeout: 5))
      XCTAssertTrue(doneButton.isSelected)
    }

    if allPlatforms { checkBrowserOpenRow() }
    try await peer("toggle")
    XCTAssertTrue(row.waitForExistence(timeout: 10))
    XCTAssertTrue(app.buttons["Reopen"].exists)
    XCTAssertTrue(doneButton.isSelected)
    XCTAssertTrue(app.staticTexts[label].exists)

    if allPlatforms {
      try await peer("reopen")
      XCTAssertTrue(row.waitForNonExistence(timeout: 10))
      XCTAssertTrue(doneButton.isSelected)
      checkBrowserOpenRow()
      try await peer("toggle")
      XCTAssertTrue(row.waitForExistence(timeout: 10))
    }

    try await peer("delete-during-disconnect")
    XCTAssertTrue(row.waitForNonExistence(timeout: 15))
    XCTAssertTrue(app.buttons["Reconnect"].waitForNonExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts[label].exists)

    if allPlatforms {
      all.tap()
      XCTAssertTrue(filters.buttons["All"].isSelected)

      try await peer("pause-browser")
      try await peer("hold-iphone-add")
      add.tap()
      XCTAssertTrue(draft.waitForExistence(timeout: 5))
      draft.tap()
      draft.typeText("iPhone peer todo\n")
      let ios = app.staticTexts["iPhone peer todo"]
      XCTAssertTrue(ios.waitForExistence(timeout: 10))
      XCTAssertTrue(app.staticTexts["Pending"].waitForExistence(timeout: 5))
      XCTAssertTrue(app.staticTexts[label].exists)
      try await peer("fail-iphone-add")
      let retry = app.buttons["Try again"]
      XCTAssertTrue(retry.waitForExistence(timeout: 10))
      try await peer("capture-iphone-retry")
      retry.tap()
      XCTAssertTrue(app.buttons["Complete"].waitForExistence(timeout: 10))
      try await peer("check-ios-add")

      app.buttons["Complete"].tap()
      XCTAssertTrue(app.buttons["Reopen"].waitForExistence(timeout: 5))
      try await peer("check-ios-complete")
      app.buttons["Reopen"].tap()
      XCTAssertTrue(app.buttons["Complete"].waitForExistence(timeout: 5))
      try await peer("check-ios-reopen")
      ios.swipeLeft()
      app.buttons["Delete"].tap()
      app.alerts.firstMatch.buttons["Confirm"].tap()
      XCTAssertTrue(ios.waitForNonExistence(timeout: 10))
      try await peer("check-ios-delete")

      let android = app.staticTexts["Android peer todo"]
      try await peer("android-pending-retry")
      XCTAssertTrue(android.waitForExistence(timeout: 10))
      XCTAssertTrue(app.buttons["Complete"].exists)
      try await peer("android-complete")
      XCTAssertTrue(app.buttons["Reopen"].waitForExistence(timeout: 10))
      try await peer("android-reopen")
      XCTAssertTrue(app.buttons["Complete"].waitForExistence(timeout: 10))
      try await peer("android-delete")
      XCTAssertTrue(android.waitForNonExistence(timeout: 10))
      XCTAssertTrue(app.staticTexts[label].exists)
    }
  }

  func testAddTaskToFreshServerStore() {
    let app = XCUIApplication()
    app.launchEnvironment["EWE_TODO_URL"] =
      ProcessInfo.processInfo.environment["EWE_TODO_URL"] ?? "http://127.0.0.1:8080"
    app.launchEnvironment["EWE_TODO_SOCKET"] = ProcessInfo.processInfo.environment["EWE_TODO_SOCKET"]
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
