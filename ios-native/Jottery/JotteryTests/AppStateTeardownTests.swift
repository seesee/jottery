import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// What's left in memory once the app has locked or sync has ended.
@MainActor
struct AppStateTeardownTests {

    private func makeUnlockedState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-teardown-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        state.keyManager.unlockWithKeyData(Data(repeating: 7, count: 32))
        state.isLocked = false
        return state
    }

    private func makeService(for state: AppState) throws -> SyncService {
        SyncService(
            syncClient: SyncClient(endpoint: "http://localhost:9"),
            noteRepo: try #require(state.noteRepo),
            syncRepo: try #require(state.syncRepo),
            versionRepo: try #require(state.versionRepo),
            attachmentRepo: try #require(state.attachmentRepo),
            savedSearchRepo: try #require(state.savedSearchRepo),
            key: try #require(state.keyManager.masterKey)
        )
    }

    @Test func lockingLeavesNoDecryptedNotesInMemory() throws {
        let state = try makeUnlockedState()
        _ = try state.createNote(content: "secret", tags: ["private"])
        try state.saveSearch(name: "mine", query: "#private")
        state.scheduleSearch()
        #expect(!state.filteredNotes.isEmpty)
        #expect(!state.savedSearches.isEmpty)

        state.lock()

        #expect(state.notes.isEmpty)
        #expect(state.filteredNotes.isEmpty)
        #expect(state.savedSearches.isEmpty)
    }

    /// A refresh decrypting when the lock fires holds an older ticket, and
    /// publishing it would put the vault back behind the lock screen.
    @Test func aRefreshAlreadyRunningCantPublishAfterTheLock() throws {
        let state = try makeUnlockedState()

        state.lock()

        var published = false
        state.publishIfNewest(ticket: 1) { published = true }
        #expect(!published)
    }

    @Test func wipingForgetsAnEditWaitingToBeSaved() throws {
        let state = try makeUnlockedState()
        state.pendingEditorNote = try state.createNote(content: "unsaved")

        state.wipeAllData()

        #expect(state.pendingEditorNote == nil)
    }

    @Test func endingSyncLetsGoOfTheSyncObjects() throws {
        let state = try makeUnlockedState()
        state.syncClient = SyncClient(endpoint: "http://localhost:9")
        state.syncService = try makeService(for: state)
        state.syncEnabled = true

        state.teardownSync()

        #expect(state.syncClient == nil)
        #expect(state.syncService == nil)
        #expect(!state.syncEnabled)
    }

    /// The live-updates listener used to keep the service alive, and with it
    /// the master key, after the app had locked.
    @Test func aStoppedSyncServiceIsReleased() async throws {
        let state = try makeUnlockedState()
        var service: SyncService? = try makeService(for: state)
        weak let released = service
        await service?.startSSE()
        await service?.stop()

        service = nil

        for _ in 0..<500 where released != nil {
            try await Task.sleep(for: .milliseconds(1))
        }
        #expect(released == nil)
    }
}
