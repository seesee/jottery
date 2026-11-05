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

    private func makeService(for state: AppState, listener: SSEClient? = nil) throws -> SyncService {
        SyncService(
            syncClient: SyncClient(endpoint: "http://localhost:9"),
            noteRepo: try #require(state.noteRepo),
            syncRepo: try #require(state.syncRepo),
            versionRepo: try #require(state.versionRepo),
            attachmentRepo: try #require(state.attachmentRepo),
            savedSearchRepo: try #require(state.savedSearchRepo),
            key: try #require(state.keyManager.masterKey),
            sseClient: listener
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

        await waitUntil { released == nil }
        #expect(released == nil)
    }

    /// Stopping isn't all that stands between the listener and the key. If the
    /// listener held the service, anything that skipped the stop would keep it.
    @Test func theListenerAloneDoesntKeepAServiceAlive() async throws {
        let state = try makeUnlockedState()
        let listener = SSEClient(syncClient: SyncClient(endpoint: "http://localhost:9"))
        var service: SyncService? = try makeService(for: state, listener: listener)
        weak let released = service
        await service?.startSSE()

        service = nil

        // Read before stopping, because stopping would free it either way.
        let wentAway = await waitUntil { released == nil }
        await listener.stop()
        #expect(wentAway)
    }

    /// Letting go isn't enough: a service still running would go on listening to
    /// the server after the lock, and nothing would be left to stop it.
    @Test func endingSyncStopsTheService() async throws {
        let state = try makeUnlockedState()
        let service = try makeService(for: state)
        state.syncService = service

        state.teardownSync()

        await waitUntil { await service.isShutDown }
        #expect(await service.isShutDown)
    }
}
