import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// SyncService.push calls markSynced with the modifiedAt of the snapshot it
/// actually pushed (ifModifiedAt). If the user edits the note again while the
/// push round-trip is in flight, modified_at moves on — and markSynced must
/// not clear needs_sync for a note whose local state no longer matches what
/// was pushed, or the edit is silently stranded as "synced" when the server
/// has never seen it.
struct MarkSyncedGuardTests {

    private func makeRepo() throws -> NoteRepository {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-marksynced-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        return NoteRepository(db: db, versionRepo: versionRepo)
    }

    @Test func editDuringPushRoundTripSurvives() throws {
        let repo = try makeRepo()
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        var note = try repo.create(content: "original", key: key)

        // Snapshot modifiedAt as it was when the push started.
        let pushedRecord = try #require(try repo.getRaw(id: note.id))
        let t1 = pushedRecord.modifiedAt

        // Simulate a user edit landing while the push is still in flight.
        note.content = "edit made during the push round-trip"
        try repo.update(note, key: key)

        try repo.markSynced(id: note.id, syncedAt: Date().iso8601, serverVersion: 2, ifModifiedAt: t1)

        let survivor = try #require(try repo.getRaw(id: note.id))
        #expect(survivor.needsSync == true)
        #expect(survivor.version == 2)
    }

    @Test func cleanPushClearsNeedsSync() throws {
        let repo = try makeRepo()
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
        let note = try repo.create(content: "original", key: key)

        let pushedRecord = try #require(try repo.getRaw(id: note.id))
        let t1 = pushedRecord.modifiedAt

        // No intervening edit: markSynced should clear needs_sync as normal.
        try repo.markSynced(id: note.id, syncedAt: Date().iso8601, serverVersion: 1, ifModifiedAt: t1)

        let survivor = try #require(try repo.getRaw(id: note.id))
        #expect(survivor.needsSync == false)
        #expect(survivor.version == 1)
    }
}
