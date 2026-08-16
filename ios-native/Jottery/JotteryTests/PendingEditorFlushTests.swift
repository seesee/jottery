import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Covers the lock/unlock contract for `pendingEditorNote`:
/// - `lock()` flushes the pending editor note while the key is available.
/// - On flush failure the note is retained (never silently dropped).
/// - The next successful unlock retries the flush.
@MainActor
struct PendingEditorFlushTests {

    private let fixedKey = Data(repeating: 7, count: 32)

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-pending-flush-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(fixedKey)
        state.isLocked = false
        return state
    }

    /// Pins existing behaviour: while the key is still available, lock()
    /// flushes the pending editor note to disk and clears it.
    @Test func lockFlushesPendingEditorNoteWhileKeyIsAvailable() throws {
        let state = try makeUnlockedState()
        var note = try #require(try state.createNote(content: "original title"))
        note.content = "edited before lock"
        state.pendingEditorNote = note

        state.lock()

        #expect(state.pendingEditorNote == nil)

        // Re-supply the key (as the next unlock would) and confirm the
        // edit actually reached the database.
        state.keyManager.unlockWithKeyData(fixedKey)
        let stored = try state.noteRepo?.get(id: note.id, key: state.keyManager.masterKey!)
        #expect(stored?.content == "edited before lock")
    }

    /// Regression test: if the flush cannot succeed (e.g. the key has
    /// already been wiped), the pending edit must be retained rather than
    /// silently discarded.
    @Test func flushPendingEditorNoteRetainsNoteWhenKeyUnavailable() throws {
        let state = try makeUnlockedState()
        var note = try #require(try state.createNote(content: "original title"))
        note.content = "edited but key vanished"
        state.pendingEditorNote = note

        // Simulate the key becoming unavailable before the flush runs.
        state.keyManager.masterKey = nil

        let result = state.flushPendingEditorNote()

        #expect(result == false)
        #expect(state.pendingEditorNote?.content == "edited but key vanished")

        // The database must still hold the original, unedited content.
        state.keyManager.unlockWithKeyData(fixedKey)
        let stored = try state.noteRepo?.get(id: note.id, key: state.keyManager.masterKey!)
        #expect(stored?.content == "original title")
    }

    /// End-to-end regression: a lock-time flush failure must not lose the
    /// edit — the next successful password unlock retries it automatically.
    @Test func unlockRetriesPendingEditorFlushAfterLockTimeFailure() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-pending-flush-unlock-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))

        let password = "correct horse battery staple"
        try await state.createVault(password: password)

        var note = try #require(try state.createNote(content: "original title"))
        note.content = "edited while key vanished"
        state.pendingEditorNote = note

        // Simulate the lock-time write failing (key unavailable) so the
        // pending edit survives lock() unpersisted.
        state.keyManager.masterKey = nil
        state.lock()

        #expect(state.pendingEditorNote?.content == "edited while key vanished")

        // A real, successful unlock should retry the flush before loading notes.
        try await state.unlock(password: password)

        #expect(state.pendingEditorNote == nil)
        #expect(state.notes.first(where: { $0.id == note.id })?.content == "edited while key vanished")
    }
}
