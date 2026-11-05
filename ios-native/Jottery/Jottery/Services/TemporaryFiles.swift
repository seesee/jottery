import Foundation

/// Files that exist only to hand something to the system: an export or backup
/// on its way to the share sheet, an attachment being previewed, a picked photo
/// on its way into the vault. Several hold decrypted data.
///
/// They all live under one folder of ours, protected so they can't be read
/// while the device is locked, and each in a folder of its own, so a filename
/// can't reach anything else. Whoever makes a file removes it when finished;
/// `removeAll()` sweeps up at launch, lock and wipe in case that never happens.
struct TemporaryFiles: Sendable {

    let root: URL

    static let shared = TemporaryFiles(
        root: FileManager.default.temporaryDirectory.appendingPathComponent("jottery-private", isDirectory: true)
    )

    /// A path for a new file named as close to `filename` as is safe.
    private func newFileURL(named filename: String) throws -> URL {
        let folder = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(
            at: folder,
            withIntermediateDirectories: true,
            attributes: [.protectionKey: FileProtectionType.complete]
        )
        return folder.appendingPathComponent(AttachmentFilename.safe(filename))
    }

    func write(_ data: Data, named filename: String) throws -> URL {
        let url = try newFileURL(named: filename)
        try data.write(to: url, options: [.atomic, .completeFileProtection])
        return url
    }

    /// An empty file, for writing to as data is produced.
    func createFile(named filename: String) throws -> URL {
        let url = try newFileURL(named: filename)
        guard FileManager.default.createFile(
            atPath: url.path,
            contents: nil,
            attributes: [.protectionKey: FileProtectionType.complete]
        ) else {
            throw CocoaError(.fileWriteUnknown)
        }
        return url
    }

    func copy(_ source: URL, named filename: String) throws -> URL {
        let url = try newFileURL(named: filename)
        try FileManager.default.copyItem(at: source, to: url)
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: url.path)
        return url
    }

    /// Remove a file made here, and the folder made for it.
    func remove(_ url: URL) {
        let folder = url.deletingLastPathComponent()
        guard folder.deletingLastPathComponent().standardizedFileURL == root.standardizedFileURL else { return }
        try? FileManager.default.removeItem(at: folder)
    }

    func removeAll() {
        try? FileManager.default.removeItem(at: root)
    }

    /// Remove what builds before this one left in the temporary folder, where
    /// exports and attachments stayed readable after a lock or a wipe.
    static func removeLeftovers(in directory: URL = FileManager.default.temporaryDirectory) {
        let fileManager = FileManager.default
        for folder in ["jottery-attachments", "jottery-import"] {
            try? fileManager.removeItem(at: directory.appendingPathComponent(folder, isDirectory: true))
        }
        let names = (try? fileManager.contentsOfDirectory(atPath: directory.path)) ?? []
        for name in names where isLeftover(name) {
            try? fileManager.removeItem(at: directory.appendingPathComponent(name))
        }
    }

    private static func isLeftover(_ name: String) -> Bool {
        (name.hasPrefix("jottery-export-") && name.hasSuffix(".json"))
            || (name.hasPrefix("jottery-backup-") && name.hasSuffix(".jottery"))
            || (name.hasPrefix("jottery-restore-") && name.hasSuffix(".jottery"))
    }
}
