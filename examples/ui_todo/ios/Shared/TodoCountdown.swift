import EweUI
import SwiftUI
import UIKit

/// Connect the shared Todo countdown to the iPhone screen lifecycle.
@MainActor final class TodoCountdown: ObservableObject {
  @Published private(set) var bridge: EventBridge?
  @Published private(set) var readyCount = 0

  func attach(_ bridge: EventBridge) {
    self.bridge = bridge
    readyCount += 1
  }

  var paused: Bool {
    bridge?.containsElement("todo-pause-modal", inView: "todos") == true
  }

  func tick(refresh: Bool) async {
    guard let bridge else { return }
    do {
      try await bridge.triggerStructured("host:tick_todo_countdown", inView: "todos", value: ["refresh": refresh])
    } catch {
      if !Task.isCancelled { bridge.reportError("Countdown", error.localizedDescription) }
    }
  }

  func run() async {
    while !Task.isCancelled {
      do { try await Task.sleep(for: .seconds(1)) }
      catch { return }
      await tick(refresh: false)
    }
  }
}

@MainActor extension View {
  func todoCountdown(_ countdown: TodoCountdown, phase: ScenePhase) -> some View {
    task(id: phase == .active ? countdown.readyCount : 0) {
      if phase == .active { await countdown.tick(refresh: true) }
    }
    .task(id: phase == .active && countdown.paused ? countdown.bridge.map(ObjectIdentifier.init) : nil) {
      if phase == .active && countdown.paused { await countdown.run() }
    }
    .onChange(of: countdown.paused) { _, paused in
      if !paused { UIAccessibility.post(notification: .announcement, argument: "Input resumed") }
    }
  }
}
