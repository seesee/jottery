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

    // SyncService.forceFullSync's deletion-processing loop (SyncService.swift ~866)
    // calls this exact repository method for the same reason as the incremental
    // pull loop above: a full resync must not let a remote deletion destroy an
    // edit the server has not seen. forceFullSync itself is only reachable through
    // the networking SyncClient, so this exercises the shared guarantee at the
    // repository level rather than duplicating a network-backed integration test.
    @Test func resyncPathKeepsNoteWithUnsyncedEdits() throws {
        let repo = try makeRepo()
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        var note = try repo.create(content: "original", key: key)
        try repo.markSynced(id: note.id, syncedAt: Date().iso8601)
        note.content = "offline edit made during a full resync window"
        try repo.update(note, key: key)   // sets needs_sync = 1

        // Mirrors SyncService.forceFullSync's deletion loop: it calls
        // hardDeleteIfSynced for each server-reported deletion.
        let deleted = try repo.hardDeleteIfSynced(id: note.id)

        #expect(deleted == false)
        let survivor = try #require(try repo.getRaw(id: note.id))
        #expect(survivor.needsSync == true)
    }
}
