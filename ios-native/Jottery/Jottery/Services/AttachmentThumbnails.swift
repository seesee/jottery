import ImageIO
import UIKit

/// Small pictures of a note's attachments, for the list under the editor.
///
/// A thumbnail shows what's inside an encrypted attachment, so it's drawn here in
/// the app, from decrypted bytes that never reach a file, and kept only in memory
/// until the vault locks.
///
/// QuickLookThumbnailing can't be used for this. It hands the work to a system
/// agent, which keeps what it draws: on the iOS 26.4 simulator, after making one
/// for a file in `TemporaryFiles` and removing the file, every pixel was still in
/// `Library/Caches/com.apple.QuickLook.thumbnailcache/thumbnails.data`, outside
/// the app and unencrypted, where a lock can't reach it.
@MainActor
final class AttachmentThumbnails {

    static let shared = AttachmentThumbnails()

    /// The longest side, in pixels. Enough for the list at any text size.
    nonisolated static let maxPixels = 192
    /// Opening a note shouldn't mean decrypting all of something bigger than this.
    /// Older builds had no limit, so it's above the one for new attachments.
    nonisolated static let maxSourceBytes = 32 * 1024 * 1024

    private let images = NSCache<NSString, UIImage>()
    /// Moves on when everything is cleared, so a picture still being drawn then
    /// isn't kept.
    private var generation = 0
    /// One at a time, because each holds a whole decrypted file.
    private var lastJob: Task<UIImage?, Never>?

    init() {
        images.countLimit = 200
    }

    /// What's already been drawn for an attachment.
    func cached(_ id: String) -> UIImage? {
        images.object(forKey: id as NSString)
    }

    /// A thumbnail, or nil if there's no picture to be had, in which case the
    /// list shows an icon. `contents` is the decrypted file, fetched off the main
    /// thread when it's this attachment's turn.
    func image(
        for attachment: AttachmentRef,
        contents: @escaping @Sendable () async throws -> Data?
    ) async -> UIImage? {
        let id = attachment.id
        if let cached = cached(id) { return cached }
        let mimeType = attachment.mimeType
        guard Self.canDraw(mimeType), attachment.size <= Self.maxSourceBytes else { return nil }

        let started = generation
        let previous = lastJob
        let job = Task.detached(priority: .utility) { () -> UIImage? in
            await previous?.value
            guard !Task.isCancelled, let data = try? await contents() else { return nil }
            return Self.draw(data, mimeType: mimeType)
        }
        lastJob = job

        let image = await withTaskCancellationHandler {
            await job.value
        } onCancel: {
            job.cancel()
        }
        guard generation == started, let image else { return nil }
        images.setObject(image, forKey: id as NSString)
        return image
    }

    func remove(_ id: String) {
        images.removeObject(forKey: id as NSString)
    }

    /// Forget every picture, including any still being drawn.
    func removeAll() {
        generation += 1
        images.removeAllObjects()
    }

    // MARK: - Drawing

    nonisolated static func canDraw(_ mimeType: String) -> Bool {
        mimeType.hasPrefix("image/") || mimeType == "application/pdf"
    }

    nonisolated static func draw(_ data: Data, mimeType: String) -> UIImage? {
        mimeType == "application/pdf" ? drawPDF(data) : drawImage(data)
    }

    private nonisolated static func drawImage(_ data: Data) -> UIImage? {
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixels,
        ]
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
            return nil
        }
        return UIImage(cgImage: image)
    }

    /// The first page. A password-protected file has nothing to show.
    private nonisolated static func drawPDF(_ data: Data) -> UIImage? {
        guard let provider = CGDataProvider(data: data as CFData),
              let document = CGPDFDocument(provider),
              document.isUnlocked,
              let page = document.page(at: 1) else {
            return nil
        }
        let box = page.getBoxRect(.cropBox)
        guard box.width > 0, box.height > 0 else { return nil }

        let isSideways = page.rotationAngle % 180 != 0
        let width = isSideways ? box.height : box.width
        let height = isSideways ? box.width : box.height
        let scale = CGFloat(maxPixels) / max(width, height)
        let size = CGSize(width: ceil(width * scale), height: ceil(height * scale))

        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        return UIGraphicsImageRenderer(size: size, format: format).image { context in
            let bounds = CGRect(origin: .zero, size: size)
            UIColor.white.setFill()
            context.fill(bounds)
            let cgContext = context.cgContext
            cgContext.translateBy(x: 0, y: size.height)
            cgContext.scaleBy(x: 1, y: -1)
            cgContext.scaleBy(x: scale, y: scale)
            let unscaled = CGRect(x: 0, y: 0, width: width, height: height)
            cgContext.concatenate(page.getDrawingTransform(.cropBox, rect: unscaled, rotate: 0, preserveAspectRatio: true))
            cgContext.drawPDFPage(page)
        }
    }
}
