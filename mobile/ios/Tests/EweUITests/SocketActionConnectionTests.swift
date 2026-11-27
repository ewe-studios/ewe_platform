#if os(iOS) || os(macOS)
import EweWire
import FlatBuffers
import Foundation
import XCTest
@testable import EweUI

@MainActor private final class ActionSocket: Channel, ConnectionAdapter {
  var events: ConnectionEvents?
  var onSend: (Data) -> Void = { _ in }
  var delivered: [Data] = []
  var failures: [String] = []
  var opens = 0

  func open(endpoint: URL, events: ConnectionEvents) throws -> any Channel {
    self.events = events
    opens += 1
    return self
  }
  func send(_ bytes: Data) async throws { onSend(bytes) }
  func close() {}

  func connection(_ manager: ConnectionManager, timeout: Duration = .seconds(2)) throws -> SocketActionConnection {
    let url = try XCTUnwrap(URL(string: "wss://example.test/socket"))
    return try SocketActionConnection(manager: manager, endpoint: url, handshakeTimeout: .seconds(2),
      actionTimeout: timeout, onReply: { [weak self] in self?.delivered.append($0) },
      onError: { [weak self] in self?.failures.append($0) })
  }
}

final class SocketActionConnectionTests: XCTestCase {
  @MainActor private func handshake(
    _ connection: SocketActionConnection, socket: ActionSocket, stream: UInt64
  ) async throws {
    let sent = expectation(description: "Send the capability request")
    let request = SubscriptionFrames.hello(connection.requestTag)
    socket.onSend = { bytes in
      XCTAssertEqual(request, bytes)
      sent.fulfill()
    }
    let opening = Task { try await connection.send(request: request) }
    await fulfillment(of: [sent], timeout: 2)
    let reply = capabilities(stream, tag: connection.requestTag)
    socket.events?.deliver(reply)
    let result = try await opening.value
    XCTAssertEqual(reply, result)
  }

  @MainActor func testCorrelatesSharedRepliesByStreamAndSequence() async throws {
    let socket = ActionSocket()
    let manager = ConnectionManager(adapters: ["websocket": socket])
    let first = try socket.connection(manager)
    let second = try socket.connection(manager)
    defer { first.close(); second.close(); manager.close() }
    try await handshake(first, socket: socket, stream: 6)
    try await handshake(second, socket: socket, stream: 7)
    XCTAssertEqual(socket.opens, 1)

    let requests = [request(6, 2), request(6, 3), request(7, 2)]
    let sent = expectation(description: "Start the three actions")
    sent.expectedFulfillmentCount = requests.count
    socket.onSend = { bytes in
      XCTAssertTrue(requests.contains(bytes))
      sent.fulfill()
    }
    let one = Task { try await first.send(request: requests[0]) }
    let two = Task { try await first.send(request: requests[1]) }
    let three = Task { try await second.send(request: requests[2]) }
    await fulfillment(of: [sent], timeout: 2)

    let replies = [acknowledge(6, 2, reject: true), acknowledge(6, 3), acknowledge(7, 2)]
    for index in [2, 1, 0] { socket.events?.deliver(replies[index]) }
    let observed = try await [one.value, two.value, three.value]
    XCTAssertEqual(replies, observed.compactMap { $0 })
    XCTAssertTrue(socket.delivered.isEmpty)
  }

  @MainActor func testTimesOutAndDeliversTheLateReply() async throws {
    let socket = ActionSocket()
    let manager = ConnectionManager(adapters: ["websocket": socket])
    let connection = try socket.connection(manager, timeout: .milliseconds(75))
    defer { connection.close(); manager.close() }
    try await handshake(connection, socket: socket, stream: 6)
    socket.onSend = { _ in }
    do {
      _ = try await connection.send(request: request(6, 2))
      XCTFail("Expected the action to time out")
    } catch let error as URLError { XCTAssertEqual(error.code, .timedOut) }
    let late = acknowledge(6, 2)
    socket.events?.deliver(late)
    XCTAssertEqual(socket.delivered, [late])
    XCTAssertTrue(socket.failures.isEmpty)
  }

  @MainActor func testCancellationReleasesTheActionWait() async throws {
    let socket = ActionSocket()
    let manager = ConnectionManager(adapters: ["websocket": socket])
    let connection = try socket.connection(manager)
    defer { connection.close(); manager.close() }
    try await handshake(connection, socket: socket, stream: 6)
    let sent = expectation(description: "Send an action before cancellation")
    let action = request(6, 2)
    socket.onSend = { bytes in
      XCTAssertEqual(bytes, action)
      sent.fulfill()
    }
    let pending = Task { try await connection.send(request: action) }
    await fulfillment(of: [sent], timeout: 2)
    pending.cancel()
    do {
      _ = try await pending.value
      XCTFail("Expected cancellation")
    } catch { XCTAssertTrue(error is CancellationError) }

    let reply = acknowledge(6, 2)
    socket.events?.deliver(reply)
    XCTAssertEqual(socket.delivered, [reply])
    XCTAssertTrue(socket.failures.isEmpty)
  }

  @MainActor func testDisconnectFailsPendingActionAndRecoveryAwaitsCaps() async throws {
    let socket = ActionSocket()
    let manager = ConnectionManager(adapters: ["websocket": socket])
    let connection = try socket.connection(manager)
    defer { connection.close(); manager.close() }
    try await handshake(connection, socket: socket, stream: 6)
    let sent = expectation(description: "Send an action before disconnect")
    socket.onSend = { _ in sent.fulfill() }
    let pending = Task { try await connection.send(request: request(6, 2)) }
    await fulfillment(of: [sent], timeout: 2)
    socket.events?.disconnected("Disconnected after sending")
    do {
      _ = try await pending.value
      XCTFail("Expected the action to fail on disconnect")
    } catch { XCTAssertTrue(String(describing: error).contains("Disconnected after sending")) }
    XCTAssertEqual(socket.failures, ["Disconnected after sending"])

    let resent = expectation(description: "Send capability request on reconnect")
    let recovered = expectation(description: "Send action after the new capabilities")
    let caps = capabilities(7, tag: connection.requestTag)
    let next = request(7, 3)
    socket.onSend = { bytes in
      if bytes == next {
        XCTAssertEqual(socket.delivered, [caps])
        recovered.fulfill()
      } else {
        XCTAssertEqual(bytes, SubscriptionFrames.hello(connection.requestTag))
        resent.fulfill()
      }
    }
    let retry = Task {
      try await connection.awaitReady()
      return try await connection.send(request: next)
    }
    socket.events?.opened()
    await fulfillment(of: [resent], timeout: 2)
    socket.events?.deliver(acknowledge(6, 2))
    XCTAssertTrue(socket.delivered.isEmpty)
    socket.events?.deliver(caps)
    await fulfillment(of: [recovered], timeout: 2)
    let reply = acknowledge(7, 3)
    socket.events?.deliver(reply)
    let result = try await retry.value
    XCTAssertEqual(result, reply)
  }
}

private func request(_ stream: UInt64, _ sequence: UInt64) -> Data {
  var builder = FlatBufferBuilder()
  let body = ewe_ui_FrameRequest.createFrameRequest(&builder)
  let frame = ewe_ui_Frame.createFrame(
    &builder, schemaVersion: 1, streamId: stream, sequenceId: sequence, kindType: .framerequest, kindOffset: body)
  ewe_ui_Frame.finish(&builder, end: frame)
  return Data(builder.sizedByteArray)
}

private func capabilities(_ stream: UInt64, tag: UInt64) -> Data {
  var builder = FlatBufferBuilder()
  let caps = ewe_ui_ServerCaps.createServerCaps(
    &builder, status: .ok, schemaVersion: 1, vocabularyVersion: 13, streamId: stream, requestTag: tag)
  let control = ewe_ui_Control.createControl(&builder, capsOffset: caps)
  return reply(&builder, stream: stream, sequence: 0, control: control)
}

private func acknowledge(_ stream: UInt64, _ sequence: UInt64, reject: Bool = false) -> Data {
  var builder = FlatBufferBuilder()
  var control = Offset()
  if reject {
    let text = builder.create(string: "Rejected")
    let error = ewe_ui_ActionError.createActionError(&builder, code: .invalidinput, messageOffset: text)
    control = ewe_ui_Control.createControl(&builder, errorOffset: error)
  }
  return reply(&builder, stream: stream, sequence: sequence, control: control)
}

private func reply(_ builder: inout FlatBufferBuilder, stream: UInt64, sequence: UInt64, control: Offset) -> Data {
  var messages = Offset()
  if !control.isEmpty {
    let message = ewe_ui_Message.createMessage(&builder, controlOffset: control)
    messages = builder.createVector(ofOffsets: [message])
  }
  let body = ewe_ui_FrameMessage.createFrameMessage(&builder, ack: sequence, messagesVectorOffset: messages)
  let frame = ewe_ui_Frame.createFrame(
    &builder, schemaVersion: 1, streamId: stream, sequenceId: 1, kindType: .framemessage, kindOffset: body)
  ewe_ui_Frame.finish(&builder, end: frame)
  return Data(builder.sizedByteArray)
}
#endif
