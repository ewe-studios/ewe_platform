import EweCore
import Foundation

/// Reads each resource from the bundle that contains this application.
public final class BundleResourceSource: HostResourceSource, @unchecked Sendable {
    private let directory: URL?

    public init(bundle: Bundle = .main) {
        directory = bundle.resourceURL?.appendingPathComponent("EweResources", isDirectory: true)
    }

    public func open(name: String) throws -> HostResourceFile? {
        guard let directory else {
            throw CoreError.Transport(detail: "The application has no resource bundle")
        }
        let url = directory.appendingPathComponent(name)
        do {
            return try BundleResourceFile(url: url)
        } catch let error as CocoaError where error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile {
            return nil
        } catch {
            throw CoreError.Transport(detail: error.localizedDescription)
        }
    }
}

private final class BundleResourceFile: HostResourceFile, @unchecked Sendable {
    private let handle: FileHandle

    init(url: URL) throws {
        handle = try FileHandle(forReadingFrom: url)
    }

    func read(capacity: UInt32) throws -> Data {
        do { return try handle.read(upToCount: Int(capacity)) ?? Data() }
        catch { throw CoreError.Transport(detail: error.localizedDescription) }
    }

    func close() throws {
        do { try handle.close() }
        catch { throw CoreError.Transport(detail: error.localizedDescription) }
    }
}
