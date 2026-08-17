import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Pins the contract for `AppState.applySyncChanges(_:)` (Task 3 of the
/// unlock-performance plan, jottery-ios-unlock-perf): a sync cycle that only
/// touched a handful of notes should refresh just those rows — not decrypt
/// and republish the whole vault via `loadNotes()`.
@MainActor
struct IncrementalRefreshTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-incremental-refresh-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    @Test func applySyncChangesRefreshesOnlyAffectedRows() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        let note1 = try noteRepo.create(content: "note one", tags: [], key: key)
        let note2 = try noteRepo.create(content: "note two", tags: [], key: key)
        let note3 = try noteRepo.create(content: "note three", tags: [], key: key)

        try await state.loadNotes()
        #expect(state.notes.count == 3)

        let originalNote1 = try #require(state.notes.first { $0.id == note1.id })

        // Simulate "server changed note 2" by mutating its record directly via
        // the repository — this mirrors what SyncService.pull() would have
        // done to the underlying row without going through a real sync cycle.
        var updatedNote2 = note2
        updatedNote2.content = "note two — updated by server"
        try noteRepo.update(updatedNote2, key: key)

        let changes = SyncChanges(updatedIds: [note2.id], deletedIds: [note3.id], fullReloadRequired: false)
        await state.applySyncChanges(changes)

        // Note 2 refreshed with the new content.
        let refreshedNote2 = try #require(state.notes.first { $0.id == note2.id })
        #expect(refreshedNote2.content == "note two — updated by server")

        // Note 3 removed from the in-memory list.
        #expect(state.notes.first { $0.id == note3.id } == nil)
        #expect(state.notes.count == 2)

        // Note 1 untouched — same value (identity by value: unchanged content).
        let untouchedNote1 = try #require(state.notes.first { $0.id == note1.id })
        #expect(untouchedNote1.content == originalNote1.content)
        #expect(untouchedNote1.modifiedAt == originalNote1.modifiedAt)
        #expect(untouchedNote1.version == originalNote1.version)

        // filteredNotes stays consistent with notes.
        let deadline = Date().addingTimeInterval(2)
        while state.filteredNotes.count != 2 && Date() < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(state.filteredNotes.count == 2)
        #expect(state.filteredNotes.contains { $0.id == note2.id && $0.content == "note two — updated by server" })
        #expect(!state.filteredNotes.contains { $0.id == note3.id })
    }

    @Test func applySyncChangesMovesNoteBetweenActiveAndArchived() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        let note = try noteRepo.create(content: "will be archived", tags: [], key: key)
        try await state.loadNotes()
        #expect(state.notes.count == 1)
        #expect(state.archivedNotes.isEmpty)

        try noteRepo.archive(id: note.id)

        await state.applySyncChanges(SyncChanges(updatedIds: [note.id]))

        #expect(state.notes.isEmpty)
        #expect(state.archivedNotes.count == 1)
        #expect(state.archivedNotes.first?.id == note.id)
    }

    @Test func applySyncChangesFallsBackToFullReloadWhenRequired() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        _ = try noteRepo.create(content: "note one", tags: [], key: key)
        _ = try noteRepo.create(content: "note two", tags: [], key: key)
        try await state.loadNotes()
        #expect(state.notes.count == 2)

        // A third note is added directly via the repo (simulating a write this
        // sync cycle couldn't attribute to a specific id) and the caller signals
        // fullReloadRequired — applySyncChanges must fall back to loadNotes().
        _ = try noteRepo.create(content: "note three — untracked", tags: [], key: key)

        await state.applySyncChanges(.fullReload)

        #expect(state.notes.count == 3)
    }

    @Test func applySyncChangesFallsBackWhenChangeSetIsOversized() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        for i in 0..<3 {
            _ = try noteRepo.create(content: "note \(i)", tags: [], key: key)
        }
        try await state.loadNotes()
        #expect(state.notes.count == 3)

        _ = try noteRepo.create(content: "note added out of band", tags: [], key: key)

        // A change set with >100 ids (none of which need to exist) should be
        // treated as ambiguous/oversized and trigger a full reload instead of
        // 101 individual decrypts.
        let hugeIds = Set((0..<101).map { "nonexistent-\($0)" })
        await state.applySyncChanges(SyncChanges(updatedIds: hugeIds))

        #expect(state.notes.count == 4)
    }

    @Test func applySyncChangesNoOpWhenChangeSetIsEmpty() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        _ = try noteRepo.create(content: "note one", tags: [], key: key)
        try await state.loadNotes()
        #expect(state.notes.count == 1)

        // A note added out of band should NOT appear — an empty SyncChanges
        // must be a true no-op, not an implicit full reload.
        _ = try noteRepo.create(content: "note added out of band", tags: [], key: key)

        await state.applySyncChanges(.none)

        #expect(state.notes.count == 1)
    }
}
