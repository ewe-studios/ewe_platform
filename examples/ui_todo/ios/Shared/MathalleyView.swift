import AVFoundation
import EweCore
import EweTodoCore
import EweUI
import SwiftUI

@MainActor final class MathalleyRoute: ObservableObject {
  @Published var visible = false
  @Published var error: String?
  private(set) var core: NativeCore?
  private weak var store: NodeStore?

  func attach(_ core: NativeCore, store: NodeStore, viewDataAction: String? = nil) throws -> NativeActionCore {
    _ = try store.registerView("todos")
    self.store = store
    self.core = core
    let adapter = NativeActionCore(core, store: store, viewDataAction: viewDataAction)
    let previous = store.onData
    store.onData = { [weak self, weak store] id, data in
      guard data.msgpack?.bytes == [0xad] + Array("open:mathalley".utf8) else {
        previous?(id, data)
        return
      }
      guard let self, let store, !visible else { return }
      do {
        _ = try store.registerView("mathalley")
        visible = true
      } catch { self.error = error.localizedDescription }
    }
    return adapter
  }

  func returned() {
    do { try store?.unregisterView("mathalley") }
    catch { self.error = error.localizedDescription }
  }
}

@MainActor final class MathalleyPlayer: ObservableObject {
  @Published var error: String?
  @Published var presented = false
  @Published var audioError: String?
  private let engine = AVAudioEngine()
  let game: MathalleyGame
  var surface: NativeDrawingSurface?
  private var link: CADisplayLink?
  private var pending = false
  private var active = false
  private var closed = false
  private var wantsSound = false
  private var interrupted = false

  init(core: NativeCore) throws {
    let rate = AVAudioSession.sharedInstance().sampleRate
    let format = AVAudioFormat(standardFormatWithSampleRate: rate > 0 ? rate : 48_000, channels: 1)!
    let seed = ProcessInfo.processInfo.environment["EWE_MATHALLEY_SEED"].flatMap(UInt64.init)
      ?? UInt64.random(in: 0...UInt64.max)
    game = try MathalleyGame(core: core, seed: seed, sampleRate: UInt32(format.sampleRate))
    let game = self.game
    let source = AVAudioSourceNode(format: format) { _, clock, count, list in
      do {
        let samples = try game.samples(atFrame: UInt64(max(0, clock.pointee.mSampleTime)), count: count)
        guard let output = UnsafeMutableAudioBufferListPointer(list).first?.mData else { return kAudio_ParamError }
        samples.withUnsafeBufferPointer { buffer in
          if let address = buffer.baseAddress { output.copyMemory(from: address, byteCount: buffer.count * MemoryLayout<Float>.stride) }
        }
        return noErr
      } catch { return kAudio_ParamError }
    }
    engine.attach(source)
    engine.connect(source, to: engine.mainMixerNode, format: format)
  }

  func visibility(_ value: Bool) {
    guard !closed else { return }
    active = value
    do {
      try game.active(value: value)
      if value && error == nil {
        if link == nil {
          let link = CADisplayLink(target: self, selector: #selector(frame(_:)))
          self.link = link
          link.add(to: .main, forMode: .common)
        }
        restartSound()
      } else {
        link?.invalidate()
        link = nil
        engine.pause()
      }
    } catch { self.error = error.localizedDescription }
  }

  @objc private func frame(_ sender: CADisplayLink) {
    guard active, !pending, let surface else { return }
    pending = true
    let time = UInt64(sender.timestamp * 1_000_000_000)
    Task {
      defer { pending = false }
      do { try await game.draw(surface: surface, timestampNanos: time); if !presented { presented = true } }
      catch { if active { self.error = error.localizedDescription; visibility(false) } }
    }
  }

  func interruption(_ value: Bool) {
    interrupted = value
    if value { engine.pause() } else { restartSound() }
  }
  private func restartSound() {
    guard wantsSound && active && !interrupted && !engine.isRunning else { return }
    do { try engine.start(); audioError = nil }
    catch { audioError = error.localizedDescription }
  }

  func close() async {
    guard !closed else { return }
    visibility(false)
    closed = true
    do { try game.stopSound() } catch { self.audioError = error.localizedDescription }
    if let surface {
      self.surface = nil
      do { try await surface.remove() } catch { self.error = error.localizedDescription }
    }
  }

  func tap(_ point: CGPoint, _ size: CGSize) {
    do {
      try game.tap(x: Float(point.x), y: Float(point.y), width: Float(size.width), height: Float(size.height))
      wantsSound = true
      restartSound()
    } catch { self.error = error.localizedDescription }
  }
}

@MainActor private final class GameStart: ObservableObject {
  let result: Result<MathalleyPlayer, Error>
  init(core: NativeCore) { result = Result { try MathalleyPlayer(core: core) } }
}

struct MathalleyScreen: View {
  let core: NativeCore
  @Environment(\.dismiss) private var dismiss
  @Environment(\.scenePhase) private var phase
  @StateObject private var start: GameStart
  init(core: NativeCore) {
    self.core = core
    _start = StateObject(wrappedValue: GameStart(core: core))
  }
  var body: some View {
    switch start.result {
    case .success(let player): MathalleyContent(core: core, player: player)
    case .failure(let error): VStack { Button("Back to Todos") { dismiss() }; Text(error.localizedDescription) }
    }
  }
}
private struct MathalleyContent: View {
  let core: NativeCore
  @Environment(\.dismiss) private var dismiss
  @Environment(\.scenePhase) private var phase
  @ObservedObject var player: MathalleyPlayer

  var body: some View {
    VStack {
      Button("Back to Todos") { Task { await player.close(); dismiss() } }
      if let error = player.audioError { Text("Sound unavailable. " + error) }
      if let error = player.error { Text(error) }
      else {
        GeometryReader { bounds in
          EweDrawingSurface(name: "mathalley", graphics: { try core.graphics() },
            changed: { player.surface = $0 }, report: { player.error = $0.localizedDescription })
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(player.presented ? "Mathalley game" : "Loading Mathalley")
            .gesture(SpatialTapGesture().onEnded { player.tap($0.location, bounds.size) })
        }
      }
    }
    .interactiveDismissDisabled()
    .onAppear { player.visibility(phase == .active) }
    .onChange(of: phase) { _, phase in player.visibility(phase == .active) }
    .onReceive(NotificationCenter.default.publisher(for: AVAudioSession.interruptionNotification)) { note in
      guard let kind = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt else { return }
      player.interruption(kind == AVAudioSession.InterruptionType.began.rawValue)
    }
    .onDisappear { player.visibility(false) }
  }
}
