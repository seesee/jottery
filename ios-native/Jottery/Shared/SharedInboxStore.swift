import Foundation

/// File-based hand-off between the share extension and the main app.
///
/// The extension never holds the master key, so items are staged
/// unencrypted in the app-group container (guarded by iOS file
/// protection) and converted into encrypted notes when the main app
/// next runs unlocked. File protection doesn't reach device backups, so the
/// directory is kept out of them.
///
/// The manifest is written last, and is what makes an item real: a directory
/// without one is an item that was abandoned partway, and gets swept.
enum SharedInboxStore {

    static let appGroupIdentifier = "group.com.jottery.ios"

    struct Manifest: Codable {
        var createdAt: Date
        var text: String?
        var urls: [String]
        var files: [File]
        /// Set once `AppState.importSharedInboxItems` successfully calls
        /// `createNote` for this item. Lets a retry after a partial failure
        /// (e.g. an attachment write throwing) resume onto the same note
        /// instead of creating a duplicate. Absent in manifests written
        /// before this field existed — the share extension never sets it,
        /// and `Codable` decodes missing optional keys as `nil`, so old
        /// staged items keep working unchanged.
        var importedNoteId: String? = nil

        struct File: Codable {
            var storedName: String   // on-disk name inside the item directory
            var filename: String     // original filename for the attachment
            var mimeType: String
        }
    }

    /// The staging directory inside the app-group container.
    static func defaultRoot() -> URL? {
        FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier)?
            .appendingPathComponent("SharedInbox", isDirectory: true)
    }

    /// One item being staged. Files go in one at a time, straight from disk,
    /// because an extension has very little memory to work with.
    struct Staging: Sendable {
        let directory: URL

        /// Copy a file in. Throws if it can't be copied, and the caller decides
        /// what to do with the rest.
        /// - Parameter protect: replaces the attribute being set. Only used by
        ///   tests, because the simulator doesn't keep it.
        func add(
            copying source: URL, index: Int, filename: String, mimeType: String,
            protect: (URL) throws -> Void = {
                try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: $0.path)
            }
        ) throws -> Manifest.File {
            let name = storedName(index: index, filename: filename)
            let destination = directory.appendingPathComponent(name)
            try FileManager.default.copyItem(at: source, to: destination)
            try protect(destination)
            return Manifest.File(storedName: name, filename: filename, mimeType: mimeType)
        }

        func add(data: Data, index: Int, filename: String, mimeType: String) throws -> Manifest.File {
            let name = storedName(index: index, filename: filename)
            try data.write(to: directory.appendingPathComponent(name), options: [.atomic, .completeFileProtection])
            return Manifest.File(storedName: name, filename: filename, mimeType: mimeType)
        }

        /// Finish the item, which makes it visible to the main app.
        func commit(text: String?, urls: [String], files: [Manifest.File]) throws {
            let manifest = Manifest(createdAt: Date(), text: text, urls: urls, files: files)
            let encoder = JSONEncoder()
            encoder.dateEncodingStrategy = .iso8601
            try (try encoder.encode(manifest)).write(
                to: directory.appendingPathComponent("manifest.json"),
                options: [.atomic, .completeFileProtection]
            )
        }

        /// Remove what's been staged. Nothing shared stays behind unencrypted.
        func discard() {
            try? FileManager.default.removeItem(at: directory)
        }

        private func storedName(index: Int, filename: String) -> String {
            let ext = (filename as NSString).pathExtension
            return ext.isEmpty ? "file-\(index)" : "file-\(index).\(ext)"
        }
    }

    enum StagingError: Error {
        /// The app group's container isn't available.
        case noContainer
    }

    /// Start staging an item. `root` is injectable for tests.
    static func begin(in root: URL? = nil) throws -> Staging {
        guard let root = root ?? defaultRoot() else { throw StagingError.noContainer }
        try prepare(root)
        let directory = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return Staging(directory: directory)
    }

    /// Stage one shared item whole. `root` is injectable for tests.
    static func write(
        text: String?,
        urls: [String],
        files: [(data: Data, filename: String, mimeType: String)],
        in root: URL? = nil
    ) throws {
        let staging = try begin(in: root)
        do {
            let staged = try files.enumerated().map { index, file in
                try staging.add(data: file.data, index: index, filename: file.filename, mimeType: file.mimeType)
            }
            try staging.commit(text: text, urls: urls, files: staged)
        } catch {
            staging.discard()
            throw error
        }
    }

    /// Create the staging directory, and keep it out of device backups.
    private static func prepare(_ root: URL) throws {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        var url = root
        try url.setResourceValues(values)
    }

    /// Remove items an extension abandoned partway: the system can kill one at
    /// any moment, and what it staged is plaintext. Runs from the main app. One
    /// younger than `age` may still be in progress. Also marks staging as
    /// excluded from backups for items staged before that was done.
    static func removeAbandoned(olderThan age: TimeInterval = 60 * 60, in root: URL? = nil) {
        guard let root = root ?? defaultRoot(),
              let entries = try? FileManager.default.contentsOfDirectory(
                  at: root, includingPropertiesForKeys: [.creationDateKey]
              ) else { return }
        try? prepare(root)
        let cutoff = Date().addingTimeInterval(-age)
        for directory in entries {
            let manifest = directory.appendingPathComponent("manifest.json")
            guard !FileManager.default.fileExists(atPath: manifest.path) else { continue }
            let created = (try? directory.resourceValues(forKeys: [.creationDateKey]))?.creationDate
            if created.map({ $0 < cutoff }) ?? true {
                remove(directory)
            }
        }
    }

    /// Remove everything staged, when the vault is wiped.
    static func removeAll(in root: URL? = nil) {
        guard let root = root ?? defaultRoot() else { return }
        try? FileManager.default.removeItem(at: root)
    }

    /// All staged items, oldest first. Entries without a readable manifest are skipped.
    static func pendingItems(in root: URL? = nil) -> [(directory: URL, manifest: Manifest)] {
        guard let root = root ?? defaultRoot(),
              let entries = try? FileManager.default.contentsOfDirectory(
                  at: root, includingPropertiesForKeys: nil
              ) else { return [] }

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return entries
            .compactMap { directory -> (URL, Manifest)? in
                guard let data = try? Data(contentsOf: directory.appendingPathComponent("manifest.json")),
                      let manifest = try? decoder.decode(Manifest.self, from: data) else { return nil }
                return (directory, manifest)
            }
            .sorted { $0.1.createdAt < $1.1.createdAt }
    }

    /// Delete a staged item after import (or on user rejection).
    static func remove(_ directory: URL) {
        try? FileManager.default.removeItem(at: directory)
    }

    /// Record the note id created for a staged item. Called immediately
    /// after `createNote` succeeds, before any attachments are processed,
    /// so a later retry (see `pendingItems`) can detect the resumable note
    /// via `Manifest.importedNoteId` instead of creating a duplicate.
    static func markImported(_ directory: URL, noteId: String) throws {
        let manifestURL = directory.appendingPathComponent("manifest.json")
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        var manifest = try decoder.decode(Manifest.self, from: try Data(contentsOf: manifestURL))
        manifest.importedNoteId = noteId

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        try (try encoder.encode(manifest)).write(
            to: manifestURL,
            options: [.atomic, .completeFileProtection]
        )
    }
}
