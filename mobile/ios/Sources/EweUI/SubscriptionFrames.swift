import EweWire
import FlatBuffers
import Foundation

enum SubscriptionFrames {
  static func hello(_ tag: UInt64) -> Data {
    var b = FlatBufferBuilder()
    let hello = ewe_ui_ClientHello.createClientHello(&b, vocabularyVersion: eweWireVocabularyVersion, platform: .ios,
      mount: .native, subscriptionResume: true, requestTag: tag)
    let request = ewe_ui_FrameRequest.createFrameRequest(&b, helloOffset: hello)
    return finish(&b, session: 0, request: request)
  }

  static func resume(session: UInt64, id: UInt64, stream: UInt64, after: UInt64) -> Data {
    var b = FlatBufferBuilder()
    let cursor = ewe_ui_ResumeSubscription.createResumeSubscription(&b, deliverySession: session,
      subscriptionId: id, streamId: stream, after: after)
    let request = ewe_ui_FrameRequest.createFrameRequest(&b, resumeOffset: cursor)
    return finish(&b, session: session, request: request)
  }

  static func expired(stream: UInt64, after: UInt64) -> Data {
    var b = FlatBufferBuilder()
    let expired = ewe_ui_ReplayExpired.createReplayExpired(&b,streamId:stream,after:after)
    let control = ewe_ui_Control.createControl(&b,replayExpiredOffset:expired)
    let message = ewe_ui_Message.createMessage(&b,controlOffset:control)
    let items = b.createVector(ofOffsets:[message])
    let body = ewe_ui_FrameMessage.createFrameMessage(&b,messagesVectorOffset:items)
    let frame = ewe_ui_Frame.createFrame(&b,schemaVersion:1,streamId:stream,kindType:.framemessage,kindOffset:body)
    ewe_ui_Frame.finish(&b,end:frame)
    return Data(b.sizedByteArray)
  }

  private static func finish(_ b: inout FlatBufferBuilder, session: UInt64, request: Offset) -> Data {
    let frame = ewe_ui_Frame.createFrame(&b, schemaVersion: 1, streamId: session, kindType: .framerequest, kindOffset: request)
    ewe_ui_Frame.finish(&b, end: frame)
    return Data(b.sizedByteArray)
  }

  static func caps(_ bytes: Data) -> ewe_ui_ServerCaps? {
    var buffer = ByteBuffer(data: bytes)
    guard let frame: ewe_ui_Frame = try? getCheckedRoot(byteBuffer: &buffer, fileId: "EWF1"),
      frame.kindType == .framemessage, let body = frame.kind(type: ewe_ui_FrameMessage.self),
      body.messagesCount > 0 else { return nil }
    return body.messages(at: 0)?.control?.caps
  }
}

@MainActor final class LiveSubscription: Channel {
  private var handle: (any ConnectionHandle)?
  private let spec: LiveSpec
  private let id: UInt64
  private let mount: UInt32
  private weak var store: NodeStore?
  private var pending: UInt64?
  private var session: UInt64?
  private var socket: Bool { spec.transport.lowercased() == "websocket" }

  init(manager: ConnectionManager, spec: LiveSpec, endpoint: URL, mount: UInt32, store: NodeStore) throws {
    let generation = store.recovery?.generation
    self.id = try manager.nextTag()
    self.spec = spec
    self.mount = mount
    self.store = store
    let latestOnly = spec.transport == "Sse" && spec.shared == true
    self.pending = !latestOnly && (store.recovery != nil || spec.resumeFrom > 0) ? spec.resumeFrom : nil
    handle = try manager.acquire(spec.transport, endpoint: endpoint, shared: spec.shared, keepAlive: spec.keepAlive,
      owner: store, accept: { [weak self] data in
        guard let self else { return false }
        if let caps = SubscriptionFrames.caps(data) { return socket && caps.requestTag == id }
        return self.store?.matches(data, stream: spec.channelID, subscription: id) ?? false
      }, onOpen: { [weak self] in
        Task { @MainActor in
          guard let self, self.socket, self.store?.subscriptionReplay == true else { return }
          do { try await self.handle?.send(SubscriptionFrames.hello(self.id)) }
          catch { self.store?.error("Transport", String(describing: error)) }
        }
      }, onContext: { [weak self] data in self?.context(data) }, onError: { [weak self] cause in
        guard let self else { return }
        session = nil
        if socket || spec.shared != true { pending = self.store?.cursor(stream: spec.channelID) }
        self.store?.error("Transport", cause)
      }, receive: { [weak self] data in
        guard let self else { return }
        if SubscriptionFrames.caps(data) != nil { context(data) }
        else if generation.map({ self.store?.recovery?.accepts($0) == true }) != false { self.store?.receive(data, at: mount) }
      })
  }

  private func context(_ bytes: Data) {
    guard let caps = SubscriptionFrames.caps(bytes), caps.subscriptionResume, !socket || caps.requestTag == id else { return }
    session = caps.streamId
    if let pending { self.pending = nil; resume(pending) }
  }

  func resume(_ cursor: UInt64) {
    guard let session else { pending = cursor; return }
    let bytes = SubscriptionFrames.resume(session: session, id: id, stream: spec.channelID, after: cursor)
    Task { @MainActor in
      do { try await handle?.send(bytes) }
      catch ChannelError.httpStatus(410) { store?.onSnapshot?(mount,spec.channelID) }
      catch { store?.error("Transport", String(describing: error)) }
    }
  }

  func close() { handle?.close(); handle = nil }
}
