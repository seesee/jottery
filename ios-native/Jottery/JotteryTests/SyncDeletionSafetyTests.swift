import CryptoKit
import Foundation
import Testing

@testable import Jottery

struct SyncDeletionSafetyTests {

    private func makeRepo() throws -> NoteRepository {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-deletion-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        return NoteRepository(db: db, versionRepo: versionRepo)
    }

    @Test func deletesNoteWithNoPendingChanges() throws {
        let repo = try makeRepo()
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        let note = try repo.create(content: "synced note", key: key)
        try repo.markSynced(id: note.id, syncedAt: Date().iso8601)

        let deleted = try repo.hardDeleteIfSynced(id: note.id)

        #expect(deleted == true)
        #expect(try repo.getRaw(id: note.id) == nil)
    }

    @Test func keepsNoteWithUnsyncedEdits() throws {
        let repo = try makeRepo()
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        var note = try repo.create(content: "original", key: key)
        try repo.markSynced(id: note.id, syncedAt: Date().iso8601)
        note.content = "offline edit the user must not lose"
        try repo.update(note, key: key)   // sets needs_sync = 1

        let deleted = try repo.hardDeleteIfSynced(id: note.id)

        #expect(deleted == false)
        let survivor = try #require(try repo.getRaw(id: note.id))
        #expect(survivor.needsSync == true)
    }
}
