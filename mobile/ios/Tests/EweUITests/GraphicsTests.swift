#if os(iOS)
import EweCore
import EweUI
import Metal
import QuartzCore
import UIKit
import XCTest

private final class UnusedTransport: HostTransport {
  func send(request: Data) async throws -> Data? {
    XCTFail("A graphics job used the remote action transport")
    return nil
  }
}
// Keep the presented texture readable so this headless package test can inspect its pixels.
private final class ReadableLayer: CAMetalLayer {
  private let lock = NSLock()
  private var texture: MTLTexture?
  override var framebufferOnly: Bool {
    get { false }
    set { super.framebufferOnly = false }
  }
  override func nextDrawable() -> CAMetalDrawable? {
    guard let drawable = super.nextDrawable() else { return nil }
    lock.withLock { texture = drawable.texture }
    return drawable
  }
  func pixel() async throws -> [UInt8] {
    let source = try XCTUnwrap(lock.withLock { texture })
    let device = source.device
    let buffer = try XCTUnwrap(device.makeBuffer(length: 256, options: .storageModeShared))
    let commands = try XCTUnwrap(device.makeCommandQueue()?.makeCommandBuffer())
    let blit = try XCTUnwrap(commands.makeBlitCommandEncoder())
    blit.copy(from: source, sourceSlice: 0, sourceLevel: 0, sourceOrigin: .init(x: 4, y: 4, z: 0),
      sourceSize: .init(width: 1, height: 1, depth: 1), to: buffer, destinationOffset: 0,
      destinationBytesPerRow: 256, destinationBytesPerImage: 256)
    blit.endEncoding()
    await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
      commands.addCompletedHandler { _ in done.resume() }
      commands.commit()
    }
    if let error = commands.error { throw error }
    let bytes = buffer.contents().assumingMemoryBound(to: UInt8.self)
    switch source.pixelFormat {
    case .bgra8Unorm, .bgra8Unorm_srgb: return [bytes[2], bytes[1], bytes[0]]
    case .rgba8Unorm, .rgba8Unorm_srgb: return [bytes[0], bytes[1], bytes[2]]
    default: throw CocoaError(.featureUnsupported)
    }
  }
}
final class GraphicsTests: XCTestCase {
  @MainActor func testComputeAndPresentOnTwoLayers() async throws {
    let core = try NativeCore.remote(platform: .ios, names: [], transport: UnusedTransport())
    let gpu = try core.graphics()
    let adapter = try await gpu.adapter()
    print("Graphics adapter: \(adapter)")
    let shader = try String(contentsOf: XCTUnwrap(Bundle.module.url(forResource: "double", withExtension: "wgsl", subdirectory: "Fixtures")), encoding: .utf8)
    func compute() async throws {
      let words = try await gpu.compute(shader: shader, words: [0, 2, 7], groups: 3)
      XCTAssertEqual(words, [1, 5, 15])
    }
    try await compute()
    func layer() -> ReadableLayer {
      let layer = ReadableLayer()
      layer.frame = CGRect(x: 0, y: 0, width: 64, height: 64)
      layer.framebufferOnly = false
      layer.drawableSize = CGSize(width: 64, height: 64)
      return layer
    }
    let left = layer(), right = layer()
    let size: UInt32 = 64
    let a = try await gpu.attach(name: "first", layer: left, width: size, height: size)
    try await a.resize(width: 1206, height: 2241)
    try await a.clear(red: 0, green: 0, blue: 0, alpha: 1)
    do {
      try await a.resize(width: UInt32.max, height: 1)
      XCTFail("The device must reject oversized targets")
    } catch GraphicsError.Unsupported {}
    try await a.resize(width: size, height: size)
    let b = try await gpu.attach(name: "second", layer: right, width: size, height: size)
    try await a.resize(width: 0, height: 0)
    do { try await a.clear(red: 0, green: 1, blue: 0, alpha: 1); XCTFail("A suspended surface presented") }
    catch GraphicsError.Suspended {}
    try await a.resize(width: size, height: size)
    try await a.clear(red: 0, green: 1, blue: 0, alpha: 1)
    try await a.remove()
    do { try await a.clear(red: 1, green: 0, blue: 0, alpha: 1); XCTFail("A stale surface presented") }
    catch GraphicsError.StaleSurface {}
    let c = try await gpu.attach(name: "first", layer: left, width: size, height: size)
    try await c.clear(red: 1, green: 0, blue: 0, alpha: 1)
    try await b.clear(red: 0, green: 0, blue: 1, alpha: 1)
    try await compute()
    let firstPixel = try await left.pixel()
    let secondPixel = try await right.pixel()
    XCTAssertEqual(firstPixel, [255, 0, 0])
    XCTAssertEqual(secondPixel, [0, 0, 255])
    try await c.remove()
    try await b.remove()
  }
}
#endif
