#if os(iOS)
import CoreImage
import EweCore
import EweUI
import Metal
import QuartzCore
import UIKit
import XCTest

private final class RemoteProbe: HostTransport {
  func send(request: Data) async throws -> Data? {
    XCTFail("The local readiness action sent a network request")
    return nil
  }
}
private final class DeviceMailbox: HostDeviceProvider, @unchecked Sendable {
  private let lock = NSLock()
  private var requests: [NativeDeviceRequest] = []
  func request(request: NativeDeviceRequest) { lock.withLock { requests.append(request) } }
  func next() throws -> NativeDeviceRequest {
    try lock.withLock { try XCTUnwrap(requests.isEmpty ? nil : requests.removeFirst()) }
  }
}
private final class Completion: HostDeviceCompletion, @unchecked Sendable {
  let finished = XCTestExpectation(description: "External graphics queue released")
  func released(deviceLost: Bool) { XCTAssertFalse(deviceLost); finished.fulfill() }
}

final class HostGraphicsTests: XCTestCase {
  @MainActor func testHostSuppliesOriginalMetalDeviceQueueAndTexture() async throws {
    let core = try NativeCore.remote(platform: .ios, names: ["foundation_gpu_ready"], transport: RemoteProbe())
    let provider = DeviceMailbox()
    XCTAssertThrowsError(try core.installDeviceProvider(readyAction: "server:foundation_gpu_ready", provider: provider))
    try core.installDeviceProvider(readyAction: "host:foundation_gpu_ready", provider: provider)
    let device = try XCTUnwrap(MTLCreateSystemDefaultDevice())
    let queue = try XCTUnwrap(device.makeCommandQueue())
    let descriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: 1, height: 1, mipmapped: false)
    descriptor.usage = [.shaderRead]
    descriptor.storageMode = .shared
    let source = try XCTUnwrap(device.makeTexture(descriptor: descriptor))
    let region = MTLRegionMake2D(0, 0, 1, 1)
    let upload = try XCTUnwrap(device.makeBuffer(length: 256, options: .storageModeShared))
    let red: [UInt8] = [255, 0, 0, 255]
    red.withUnsafeBytes { bytes in
      upload.contents().copyMemory(from: bytes.baseAddress!, byteCount: bytes.count)
    }
    let shader = try fixture("red_to_blue")
    let double = try fixture("double")

    let ticket = try core.requestHostDevice(device: "editor", surface: "preview")
    let request = try provider.next()
    XCTAssertEqual(request.device(), "editor")
    XCTAssertEqual(request.surface(), "preview")

    // Leave the upload on the original queue. Import must wait for it before processing the texture.
    let commands = try XCTUnwrap(queue.makeCommandBuffer())
    let blit = try XCTUnwrap(commands.makeBlitCommandEncoder())
    blit.copy(from: upload, sourceOffset: 0, sourceBytesPerRow: 256, sourceBytesPerImage: 256,
      sourceSize: .init(width: 1, height: 1, depth: 1), to: source, destinationSlice: 0,
      destinationLevel: 0, destinationOrigin: .init(x: 0, y: 0, z: 0))
    blit.endEncoding()
    commands.commit()
    let returned = Completion()
    try await request.ready(grant: .metal(device: device, queue: queue, texture: source), generation: 1, completion: returned)
    let lease = try await ticket.wait()
    try await lease.processTexture(shader: shader, x: 1, y: 1, z: 1)
    lease.release()
    await fulfillment(of: [returned.finished], timeout: 10)
    XCTAssertEqual(commands.status, .completed)
    var bytes = [UInt8](repeating: 0, count: 4)
    bytes.withUnsafeMutableBytes {
      source.getBytes($0.baseAddress!, bytesPerRow: 4, from: region, mipmapLevel: 0)
    }
    XCTAssertEqual(bytes, [0, 0, 255, 255])
    try await presentPreview(source, queue: queue)

    let computeTicket = try core.requestHostDevice(device: "editor", surface: nil)
    let computeRequest = try provider.next()
    let done = Completion()
    XCTAssertNil(computeRequest.surface())
    try await computeRequest.ready(grant: .metal(device: device, queue: queue), generation: 1, completion: done)
    let compute = try await computeTicket.wait()
    let result = try await compute.compute(shader: double, words: [0, 2, 7], groups: 3)
    XCTAssertEqual(result, [1, 5, 15])
    compute.release()
    await fulfillment(of: [done.finished], timeout: 10)
    XCTAssertTrue(source.device === device)
  }

  func testHostRefusalReachesPendingCallerThroughLocalAction() async throws {
    let core = try NativeCore.remote(platform: .ios, names: ["foundation_gpu_ready"], transport: RemoteProbe())
    let provider = DeviceMailbox()
    try core.installDeviceProvider(readyAction: "host:foundation_gpu_ready", provider: provider)
    let ticket = try core.requestHostDevice(device: "unavailable", surface: nil)
    let request = try provider.next()
    try await request.fail(reason: .denied)
    do {
      _ = try await ticket.wait()
      XCTFail("The host denied the device request")
    } catch GraphicsError.Denied {}
  }

  @MainActor private func presentPreview(_ texture: MTLTexture, queue: MTLCommandQueue) async throws {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 64, height: 64))
    let controller = UIViewController()
    window.rootViewController = controller
    let layer = CAMetalLayer()
    layer.device = texture.device
    layer.pixelFormat = .bgra8Unorm
    layer.framebufferOnly = false
    layer.frame = window.bounds
    layer.drawableSize = CGSize(width: 64, height: 64)
    controller.view.layer.addSublayer(layer)
    window.isHidden = false
    defer { window.isHidden = true }

    let drawable = try XCTUnwrap(layer.nextDrawable())
    let commands = try XCTUnwrap(queue.makeCommandBuffer())
    let colorSpace = CGColorSpaceCreateDeviceRGB()
    let image = try XCTUnwrap(CIImage(mtlTexture: texture, options: [.colorSpace: colorSpace]))
    let preview = image.transformed(by: .init(scaleX: 64, y: 64))
    let context = CIContext(mtlCommandQueue: queue)
    context.render(preview, to: drawable.texture, commandBuffer: commands, bounds: preview.extent, colorSpace: colorSpace)

    let readback = try XCTUnwrap(texture.device.makeBuffer(length: 256, options: .storageModeShared))
    let blit = try XCTUnwrap(commands.makeBlitCommandEncoder())
    blit.copy(from: drawable.texture, sourceSlice: 0, sourceLevel: 0, sourceOrigin: .init(x: 32, y: 32, z: 0),
      sourceSize: .init(width: 1, height: 1, depth: 1), to: readback, destinationOffset: 0,
      destinationBytesPerRow: 256, destinationBytesPerImage: 256)
    blit.endEncoding()
    commands.present(drawable)
    await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
      commands.addCompletedHandler { _ in done.resume() }
      commands.commit()
    }
    if let error = commands.error { throw error }
    let bgra = UnsafeBufferPointer(start: readback.contents().assumingMemoryBound(to: UInt8.self), count: 4)
    XCTAssertEqual(Array(bgra), [255, 0, 0, 255])
  }

  private func fixture(_ name: String) throws -> String {
    let url = try XCTUnwrap(Bundle.module.url(forResource: name, withExtension: "wgsl", subdirectory: "Fixtures"))
    return try String(contentsOf: url, encoding: .utf8)
  }
}
#endif
