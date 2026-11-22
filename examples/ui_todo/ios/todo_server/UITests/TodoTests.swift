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
