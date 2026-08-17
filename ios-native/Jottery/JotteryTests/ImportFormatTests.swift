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
        let exportData = try ExportService.exportAll(notes: [decrypted], attachmentRepo: attachmentRepo, key: key)

        let export = try ImportService.parse(exportData)
        let exportNote = try #require(export.notes.first)
        #expect(exportNote.pinned == true)
        #expect(exportNote.archived == true)
        #expect(exportNote.locked == true)
        #expect(exportNote.wordWrap == false)
        #expect(exportNote.syntaxLanguage == "python")
        #expect(exportNote.showPreview == true)
        #expect(exportNote.color == "blue")

        try noteRepo.hardDelete(id: created.id)
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
}
