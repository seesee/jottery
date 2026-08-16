import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// Pins the async contract for `AppState.loadNotes()` (jottery-bzar): the
/// decrypt runs off the main thread, but the method still publishes both
/// note arrays and schedules a search refresh before it returns.
@MainActor
struct AsyncLoadNotesTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-async-load-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    @Test func loadNotesPublishesBothArraysAndSchedulesSearch() async throws {
        let state = try makeUnlockedState()
        let noteRepo = try #require(state.noteRepo)
        let key = try #require(state.keyManager.masterKey)

        // Seed two notes directly via the repository — loadNotes() should
        // pick these up from the database, not rely on in-memory state.
        _ = try noteRepo.create(content: "first note", tags: [], key: key)
        _ = try noteRepo.create(content: "second note", tags: [], key: key)

        #expect(state.notes.isEmpty)

        try await state.loadNotes()

        #expect(state.notes.count == 2)
        #expect(state.archivedNotes.isEmpty)

        // scheduleSearch() resolves synchronously when searchQuery is empty,
        // but poll (bounded, no arbitrary sleeps beyond ~2s) to stay robust
        // if that debounce behaviour ever changes.
        let deadline = Date().addingTimeInterval(2)
        while state.filteredNotes.count != 2 && Date() < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(state.filteredNotes.count == 2)
    }
}
