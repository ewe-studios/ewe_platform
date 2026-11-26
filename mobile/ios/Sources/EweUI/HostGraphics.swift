#if os(iOS) || os(macOS)
import EweCore
import EweGraphicsBridge
import Metal

extension NativeDeviceGrant {
  /// Retain the original Metal device, queue, and optional initialized texture.
  /// Give the lease exclusive use of the queue and texture until its completion callback runs.
  public static func metal(device: MTLDevice, queue: MTLCommandQueue, texture: MTLTexture? = nil) throws -> NativeDeviceGrant {
    guard queue.device === device, texture.map({ $0.device === device }) ?? true else {
      throw GraphicsError.IncompatibleSurface
    }
    let handle = withExtendedLifetime((device, queue, texture)) {
      ewe_ui_gpu_retain_metal_device(
        Unmanaged.passUnretained(device as AnyObject).toOpaque(),
        Unmanaged.passUnretained(queue as AnyObject).toOpaque(),
        texture.map { Unmanaged.passUnretained($0 as AnyObject).toOpaque() }
      )
    }
    guard handle != 0 else { throw GraphicsError.IncompatibleSurface }
    return try FfiConverterTypeNativeDeviceGrant_lift(handle)
  }
}
#endif
