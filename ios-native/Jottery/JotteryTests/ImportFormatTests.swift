import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// The documented cross-platform export format (CLAUDE.md) omits fields that
/// are false/absent for a given note — `archived`, `locked`, `pinned`, `color`,
/// `syntaxLanguage`, `wordWrap`, `showPreview` and even `attachments` may all
/// be missing from a note object. `ExportNote` previously required `archived`
/// and `locked`, so a single note missing either key aborted the entire
/// import (real demo-pack files trip this). `ExportNote.init(from:)` must
/// default every genuinely-optional field instead of failing to decode.
struct ImportFormatTests {

    private func makeServices() throws -> (NoteRepository, AttachmentRepository, SymmetricKey) {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-import-format-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let noteRepo = NoteRepository(db: db, versionRepo: versionRepo)
        let attachmentRepo = AttachmentRepository(db: db)
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        return (noteRepo, attachmentRepo, key)
    }

    /// Mirrors a real demo-pack note (see demo-generation/jottery-demo-notes-en-GB.json):
    /// only id/createdAt/modifiedAt/content/tags/attachments/pinned are present.
    private let minimalNoteJSON = """
    {
      "version": "1.0",
      "exportDate": "2026-01-21T10:00:00Z",
      "notes": [
        {
          "id": "en-gb-a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
          "createdAt": "2026-01-21T10:00:00Z",
          "modifiedAt": "2026-01-21T10:00:00Z",
          "content": "# Welcome to Jottery!",
          "tags": ["welcome", "demo", "getting-started"],
          "attachments": [],
          "pinned": true
        }
      ]
    }
    """

    @Test func parsesAndImportsNoteMissingArchivedAndLockedKeys() throws {
        let (noteRepo, attachmentRepo, key) = try makeServices()
        let data = Data(minimalNoteJSON.utf8)

        let export = try ImportService.parse(data)
        let note = try #require(export.notes.first)
        #expect(note.archived == false)
        #expect(note.locked == false)
        #expect(note.wordWrap == nil)
        #expect(note.syntaxLanguage == nil)
        #expect(note.showPreview == nil)
        #expect(note.color == nil)

        let result = try ImportService.importNotes(
            export, strategy: .replace, noteRepo: noteRepo, attachmentRepo: attachmentRepo, key: key
        )
        #expect(result.imported == 1)
        #expect(result.errors.isEmpty)

        let raw = try #require(try noteRepo.getRaw(id: "en-gb-a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d"))
        #expect(raw.archived == false)
        #expect(raw.locked == false)
        #expect(raw.pinned == true)
    }

    @Test func parsesAndImportsNoteMissingTagsKey() throws {
        let (noteRepo, attachmentRepo, key) = try makeServices()
        let noteWithoutTags = """
        {
          "version": "1.0",
          "exportDate": "2026-01-21T10:00:00Z",
          "notes": [
            {
              "id": "test-no-tags-12345678-1234-1234-1234-123456789012",
              "createdAt": "2026-01-21T10:00:00Z",
              "modifiedAt": "2026-01-21T10:00:00Z",
              "content": "Note without tags key",
              "attachments": [],
              "pinned": false
            }
          ]
        }
        """
        let data = Data(noteWithoutTags.utf8)

        let export = try ImportService.parse(data)
        let note = try #require(export.notes.first)
        #expect(note.tags == [])

        let result = try ImportService.importNotes(
            export, strategy: .replace, noteRepo: noteRepo, attachmentRepo: attachmentRepo, key: key
        )
        #expect(result.imported == 1)
        #expect(result.errors.isEmpty)

        let raw = try #require(try noteRepo.getRaw(id: "test-no-tags-12345678-1234-1234-1234-123456789012"))
        // Tags should be empty when the key is missing
        let decrypted = try #require(try noteRepo.get(id: "test-no-tags-12345678-1234-1234-1234-123456789012", key: key))
        #expect(decrypted.tags == [])
    }

    @Test func fullFeaturedNoteRoundTripsExportThenImportUnchanged() throws {
        let (noteRepo, attachmentRepo, key) = try makeServices()

        let created = try noteRepo.create(content: "Full note", tags: ["a", "b"], key: key)
        var record = try #require(try noteRepo.getRaw(id: created.id))
        record.pinned = true
        record.archived = true
        record.archivedAt = Date().iso8601
        record.locked = true
        record.lockedAt = Date().iso8601
        record.wordWrap = false
        record.syntaxLanguage = "python"
        record.showPreview = true
        record.color = "blue"
        try noteRepo.updateRaw(record)

        // Use `get`, not `listActive` — the note is archived, and `listActive`
        // excludes archived notes by design.
        let decrypted = try #require(try noteRepo.get(id: created.id, key: key))
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("jottery-export-test-\(UUID().uuidString).json")
        #expect(FileManager.default.createFile(atPath: url.path, contents: nil))
        defer { try? FileManager.default.removeItem(at: url) }
        try ExportService.writeExport(notes: [decrypted], to: url, attachmentRepo: attachmentRepo, key: key)

        let export = try ImportService.parse(Data(contentsOf: url))
        let exportNote = try #require(export.notes.first)
        #expect(exportNote.pinned == true)
        #expect(exportNote.archived == true)
        #expect(exportNote.locked == true)
        #expect(exportNote.wordWrap == false)
        #expect(exportNote.syntaxLanguage == "python")
        #expect(exportNote.showPreview == true)
        #expect(exportNote.color == "blue")

        try noteRepo.permanentlyDelete(id: created.id)
        let result = try ImportService.importNotes(
            export, strategy: .replace, noteRepo: noteRepo, attachmentRepo: attachmentRepo, key: key
        )
        #expect(result.imported == 1)

        let reimported = try #require(try noteRepo.getRaw(id: created.id))
        #expect(reimported.pinned == true)
        #expect(reimported.archived == true)
        #expect(reimported.locked == true)
        #expect(reimported.wordWrap == false)
        #expect(reimported.syntaxLanguage == "python")
        #expect(reimported.showPreview == true)
        #expect(reimported.color == "blue")
    }

    // MARK: - Replacing a note (jottery-bhp5)

    private func makeRepos() throws -> (DatabaseManager, NoteRepository, AttachmentRepository) {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-import-replace-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        return (db, NoteRepository(db: db, versionRepo: VersionRepository(db: db)), AttachmentRepository(db: db))
    }

    private func exportData(replacing id: String, content: String) throws -> ExportService.ExportData {
        let json = """
            {"version":"1.0","exportDate":"2026-11-05T09:00:00Z","notes":[{"id":"\(id)",
            "createdAt":"2026-01-01T00:00:00Z","modifiedAt":"2026-01-02T00:00:00Z","content":"\(content)",
            "tags":["imported"],"attachments":[],"pinned":false}]}
            """
        return try ImportService.parse(Data(json.utf8))
    }

    /// The original used to be deleted first, and a failure after that lost it.
    @Test func aReplacementThatFailsLeavesTheOriginal() throws {
        let (db, repo, attachmentRepo) = try makeRepos()
        let key = SymmetricKey(data: Data(repeating: 9, count: 32))
        let original = try repo.create(content: "the original", key: key)
        try db.dbPool.write { database in
            // Both, because an insert is what the old order would have tried.
            try database.execute(sql: """
                CREATE TRIGGER no_updates BEFORE UPDATE ON notes
                BEGIN SELECT RAISE(ABORT, 'disk full'); END;
                CREATE TRIGGER no_inserts BEFORE INSERT ON notes
                BEGIN SELECT RAISE(ABORT, 'disk full'); END;
            """)
        }

        let result = try ImportService.importNotes(
            exportData(replacing: original.id, content: "the replacement"),
            strategy: .replace, noteRepo: repo, attachmentRepo: attachmentRepo, key: key
        )

        #expect(result.imported == 0)
        #expect(result.errors.count == 1)
        #expect(try repo.get(id: original.id, key: key)?.content == "the original")
    }

    /// Puts one attachment on a note, and returns the id of its blob.
    private func attachBlob(
        to id: String, repo: NoteRepository, attachmentRepo: AttachmentRepository
    ) throws -> String {
        let blobId = UUID().uuidString.lowercased()
        try attachmentRepo.storeBlob(id: blobId, mimeType: "text/plain", size: 1, data: Data("x".utf8))
        var record = try #require(try repo.getRaw(id: id))
        let ref = AttachmentRef(id: UUID().uuidString, filename: "old.txt", mimeType: "text/plain", size: 1, data: blobId)
        record.attachments = String(decoding: try JSONEncoder().encode([ref]), as: UTF8.self)
        try repo.updateRaw(record)
        return blobId
    }

    /// The version kept from before the import still lists what the original
    /// had, and restoring it would find nothing if the file had gone.
    @Test func aReplacementKeepsTheFilesItsVersionStillNeeds() throws {
        let (_, repo, attachmentRepo) = try makeRepos()
        let key = SymmetricKey(data: Data(repeating: 9, count: 32))
        let original = try repo.create(content: "the original", key: key)
        let blobId = try attachBlob(to: original.id, repo: repo, attachmentRepo: attachmentRepo)

        let result = try ImportService.importNotes(
            exportData(replacing: original.id, content: "the replacement"),
            strategy: .replace, noteRepo: repo, attachmentRepo: attachmentRepo, key: key
        )

        #expect(result.imported == 1)
        #expect(try attachmentRepo.hasBlob(id: blobId))
    }

    /// Nothing can reach a file no note or version lists, and it would stay on
    /// disk for ever.
    @Test func replacingANoteRemovesAFileNothingElseNeeds() throws {
        let (_, repo, attachmentRepo) = try makeRepos()
        let key = SymmetricKey(data: Data(repeating: 9, count: 32))
        let original = try repo.create(content: "the original", key: key)
        let blobId = try attachBlob(to: original.id, repo: repo, attachmentRepo: attachmentRepo)
        var replacement = try #require(try repo.getRaw(id: original.id))
        replacement.attachments = "[]"

        try repo.replace(with: replacement)

        #expect(try !attachmentRepo.hasBlob(id: blobId))
    }

    @Test func aReplacementCarriesOnFromTheNoteItReplaces() throws {
        let (_, repo, attachmentRepo) = try makeRepos()
        let key = SymmetricKey(data: Data(repeating: 9, count: 32))
        let original = try repo.create(content: "the original", key: key)
        try repo.markSynced(id: original.id, syncedAt: Date().iso8601)
        let before = try #require(try repo.getRaw(id: original.id))

        _ = try ImportService.importNotes(
            exportData(replacing: original.id, content: "the replacement"),
            strategy: .replace, noteRepo: repo, attachmentRepo: attachmentRepo, key: key
        )

        let after = try #require(try repo.getRaw(id: original.id))
        let chain = try JSONDecoder().decode([String].self, from: Data(try #require(after.hashChain).utf8))
        #expect(after.parentHash == before.contentHash)
        #expect(chain.contains(try #require(before.contentHash)))
        #expect(after.needsSync)
        #expect(try repo.get(id: original.id, key: key)?.content == "the replacement")
        // What was replaced can still be got back.
        #expect(try repo.versionRepo.getVersions(noteId: original.id).map(\.reason) == ["pre-overwrite"])
    }
}
