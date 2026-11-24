#if os(iOS)
import EweCore
import EweGraphicsBridge
import QuartzCore
import SwiftUI
import UIKit

extension NativeGraphics {
  /// Attach an existing Metal layer. Supply its drawable dimensions in pixels.
  @MainActor public func attach(name: String, layer: CAMetalLayer, width: UInt32, height: UInt32) async throws -> NativeDrawingSurface {
    let handle = ewe_ui_gpu_retain_metal_layer(Unmanaged.passUnretained(layer).toOpaque())
    guard handle != 0 else { throw CocoaError(.featureUnsupported) }
    let target = try FfiConverterTypeNativeDrawingTarget_lift(handle)
    return try await attach(name: name, target: target, width: width, height: height)
  }
}

/// Bind one drawing view to a named surface on the host device.
@MainActor public struct EweDrawingSurface: UIViewRepresentable {
  let name: String
  let graphics: () throws -> NativeGraphics
  let changed: (NativeDrawingSurface?) -> Void
  let report: (Error) -> Void
  public init(name: String, graphics: @escaping () throws -> NativeGraphics, changed: @escaping (NativeDrawingSurface?) -> Void = { _ in }, report: @escaping (Error) -> Void) {
    self.name = name
    self.graphics = graphics
    self.changed = changed
    self.report = report
  }
  public func makeUIView(context: Context) -> DrawingView { DrawingView() }
  public func updateUIView(_ view: DrawingView, context: Context) {
    view.configure(name, graphics, changed, report)
  }
  public static func dismantleUIView(_ view: DrawingView, coordinator: ()) { view.releaseSurface() }

  @MainActor public final class DrawingView: UIView {
    public override class var layerClass: AnyClass { CAMetalLayer.self }
    private var name: String?
    private var provider: (() throws -> NativeGraphics)?
    private var report: ((Error) -> Void)?
    private var changed: ((NativeDrawingSurface?) -> Void)?
    private var surface: NativeDrawingSurface?
    private var job: Task<Void, Never>?
    private var lastSize: CGSize?
    fileprivate func configure(_ name: String, _ provider: @escaping () throws -> NativeGraphics, _ changed: @escaping (NativeDrawingSurface?) -> Void, _ report: @escaping (Error) -> Void) {
      if self.name != name { releaseSurface() }
      self.name = name
      self.provider = provider
      self.changed = changed
      self.report = report
      setNeedsLayout()
    }
    public override func didMoveToWindow() {
      super.didMoveToWindow()
      setNeedsLayout()
    }
    public override func layoutSubviews() {
      super.layoutSubviews()
      guard let name, let provider, let layer = self.layer as? CAMetalLayer else { return }
      let scale = window?.screen.scale ?? traitCollection.displayScale
      let size = window == nil ? .zero : CGSize(width: (bounds.width * scale).rounded(), height: (bounds.height * scale).rounded())
      guard lastSize != size else { return }
      lastSize = size
      layer.contentsScale = scale
      layer.drawableSize = size
      let previous = job
      job = Task { [self] in
        await previous?.value
        do {
          let width = UInt32(clamping: Int(size.width)), height = UInt32(clamping: Int(size.height))
          if let surface { try await surface.resize(width: width, height: height) }
          else { surface = try await provider().attach(name: name, layer: layer, width: width, height: height); changed?(surface) }
        } catch { report?(error) }
      }
    }
    fileprivate func releaseSurface() {
      changed?(nil)
      name = nil
      lastSize = nil
      let previous = job
      job = Task { [self] in
        await previous?.value
        if let surface {
          do { try await surface.remove() }
          catch GraphicsError.StaleSurface { /* The view owner already removed this surface. */ }
          catch { report?(error) }
          self.surface = nil
        }
      }
    }
  }
}
#endif
