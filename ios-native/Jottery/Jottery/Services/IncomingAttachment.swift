import Foundation
import UniformTypeIdentifiers

/// How big an attachment may be, as on the web.
enum AttachmentLimits {
    static let maxBytes = 10 * 1024 * 1024
}

/// Why something didn't become an attachment.
enum AttachmentImportError: Error, Equatable {
    case tooLarge
    case empty
    case unreadable
}

/// Something picked, pasted or dropped, on its way into a note.
struct IncomingAttachment: Sendable {

    enum Contents: Sendable {
        case data(Data)
        /// A file outside the app, reached through the document picker.
        case pickedFile(URL)
        /// A copy in `TemporaryFiles`, which goes once it's been dealt with.
        case temporaryFile(URL, TemporaryFiles)
        /// It never got as far as being read.
        case refused(AttachmentImportError)
    }

    let filename: String
    let mimeType: String
    let contents: Contents

    /// A file from the document picker.
    init(pickedFile url: URL) {
        filename = url.lastPathComponent
        mimeType = Self.mimeType(forFilename: url.lastPathComponent)
        contents = .pickedFile(url)
    }

    init(filename: String, mimeType: String, contents: Contents) {
        self.filename = filename
        self.mimeType = mimeType
        self.contents = contents
    }

    /// For a file that goes away when the callback handing it over returns, as
    /// the photo picker's and a drop's do. One over the limit isn't copied.
    static func copying(
        _ url: URL,
        filename: String,
        mimeType: String,
        files: TemporaryFiles = .shared
    ) -> IncomingAttachment {
        let contents: Contents
        do {
            try checkFileSize(url)
            contents = .temporaryFile(try files.copy(url, named: filename), files)
        } catch let error as AttachmentImportError {
            contents = .refused(error)
        } catch {
            contents = .refused(.unreadable)
        }
        return IncomingAttachment(filename: filename, mimeType: mimeType, contents: contents)
    }

    static func mimeType(forFilename filename: String, fallback: String = "application/octet-stream") -> String {
        UTType(filenameExtension: (filename as NSString).pathExtension)?.preferredMIMEType ?? fallback
    }

    /// The bytes, or why there aren't any to use.
    func read() throws -> Data {
        switch contents {
        case .data(let data):
            return try Self.checked(data)
        case .temporaryFile(let url, _):
            return try Self.readFile(url)
        case .pickedFile(let url):
            // False for a file the app can reach anyway, which is still worth trying.
            let isScoped = url.startAccessingSecurityScopedResource()
            defer { if isScoped { url.stopAccessingSecurityScopedResource() } }
            return try Self.readFile(url)
        case .refused(let error):
            throw error
        }
    }

    /// Let go of any copy made on the way in.
    func discard() {
        if case .temporaryFile(let url, let files) = contents {
            files.remove(url)
        }
    }

    private static func readFile(_ url: URL) throws -> Data {
        try checkFileSize(url)
        guard let data = try? Data(contentsOf: url) else { throw AttachmentImportError.unreadable }
        return try checked(data)
    }

    /// Measured first, so a long video is refused without being read into memory.
    private static func checkFileSize(_ url: URL) throws {
        if let size = try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize, size > AttachmentLimits.maxBytes {
            throw AttachmentImportError.tooLarge
        }
    }

    private static func checked(_ data: Data) throws -> Data {
        if data.count > AttachmentLimits.maxBytes { throw AttachmentImportError.tooLarge }
        if data.isEmpty { throw AttachmentImportError.empty }
        return data
    }
}
