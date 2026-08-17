import Foundation

/// File-based hand-off between the share extension and the main app.
///
/// The extension never holds the master key, so items are staged
/// unencrypted in the app-group container (guarded by iOS file
/// protection) and converted into encrypted notes when the main app
/// next runs unlocked.
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

    /// Stage one shared item. `root` is injectable for tests.
    static func write(
        text: String?,
        urls: [String],
        files: [(data: Data, filename: String, mimeType: String)],
        in root: URL? = nil
    ) throws {
        guard let root = root ?? defaultRoot() else { return }
        let directory = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

        var manifestFiles: [Manifest.File] = []
        for (index, file) in files.enumerated() {
            let ext = (file.filename as NSString).pathExtension
            let storedName = ext.isEmpty ? "file-\(index)" : "file-\(index).\(ext)"
            try file.data.write(
                to: directory.appendingPathComponent(storedName),
                options: [.atomic, .completeFileProtection]
            )
            manifestFiles.append(Manifest.File(storedName: storedName, filename: file.filename, mimeType: file.mimeType))
        }

        let manifest = Manifest(createdAt: Date(), text: text, urls: urls, files: manifestFiles)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        try (try encoder.encode(manifest)).write(
            to: directory.appendingPathComponent("manifest.json"),
            options: [.atomic, .completeFileProtection]
        )
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
